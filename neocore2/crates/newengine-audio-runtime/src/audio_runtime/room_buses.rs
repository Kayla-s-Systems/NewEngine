const MAX_SHARED_ROOM_BUSES: usize = 16;
const ROOM_BUS_INGRESS_FRAMES: usize = 128;
const ROOM_BUS_SEND_LEAD_FRAMES: u64 = 2;
const NO_ROOM_BUS_SLOT: u32 = u32::MAX;

#[inline]
fn pack_room_bus_sample(frame_tag: u32, sample: f32) -> u64 {
    (u64::from(frame_tag) << 32) | u64::from(sample.to_bits())
}

#[inline]
fn unpack_room_bus_sample(word: u64) -> (u32, f32) {
    ((word >> 32) as u32, f32::from_bits(word as u32))
}

/// Lock-free many-voice -> one-room ingress. Voices write a small fixed number of frames ahead of
/// the room processor so rodio source iteration order cannot decide whether a send lands this frame
/// or the next one. Each tagged cell accumulates with a CAS and is consumed exactly once.
struct LateBusIngress {
    generation: AtomicU32,
    frame_clock: AtomicU64,
    cells: Box<[AtomicU64]>,
}

impl LateBusIngress {
    fn new() -> Self {
        let empty = pack_room_bus_sample(u32::MAX, 0.0);
        Self {
            generation: AtomicU32::new(1),
            frame_clock: AtomicU64::new(0),
            cells: (0..ROOM_BUS_INGRESS_FRAMES)
                .map(|_| AtomicU64::new(empty))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[inline]
    fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    fn reset_for_reuse(&self) -> u32 {
        let next = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.frame_clock.store(0, Ordering::Release);
        let empty = pack_room_bus_sample(u32::MAX, 0.0);
        for cell in self.cells.iter() {
            cell.store(empty, Ordering::Release);
        }
        next
    }

    #[inline]
    fn push(&self, sample: f32) {
        if !sample.is_finite() || sample.abs() <= f32::EPSILON {
            return;
        }
        let target = self
            .frame_clock
            .load(Ordering::Acquire)
            .wrapping_add(ROOM_BUS_SEND_LEAD_FRAMES) as u32;
        let cell = &self.cells[target as usize % self.cells.len()];
        let mut observed = cell.load(Ordering::Acquire);
        loop {
            let (tag, current) = unpack_room_bus_sample(observed);
            let base = if tag == target && current.is_finite() {
                current
            } else {
                0.0
            };
            let next = (base + sample).clamp(-32.0, 32.0);
            let replacement = pack_room_bus_sample(target, next);
            match cell.compare_exchange_weak(
                observed,
                replacement,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => observed = actual,
            }
        }
    }

    #[inline]
    fn take_current_frame(&self) -> f32 {
        let frame = self.frame_clock.load(Ordering::Acquire) as u32;
        let cell = &self.cells[frame as usize % self.cells.len()];
        let mut observed = cell.load(Ordering::Acquire);
        loop {
            let (tag, sample) = unpack_room_bus_sample(observed);
            if tag != frame {
                return 0.0;
            }
            let replacement = pack_room_bus_sample(frame, 0.0);
            match cell.compare_exchange_weak(
                observed,
                replacement,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return if sample.is_finite() { sample } else { 0.0 },
                Err(actual) => observed = actual,
            }
        }
    }

    #[inline]
    fn advance_frame(&self) {
        self.frame_clock.fetch_add(1, Ordering::Release);
    }
}

struct RoomBusIngressRegistry {
    slots: [Arc<LateBusIngress>; MAX_SHARED_ROOM_BUSES],
    binding_counts: [AtomicU32; MAX_SHARED_ROOM_BUSES],
}

impl RoomBusIngressRegistry {
    fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| Arc::new(LateBusIngress::new())),
            binding_counts: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    #[inline]
    fn get(&self, slot: u32) -> Option<&Arc<LateBusIngress>> {
        self.slots.get(slot as usize)
    }

    fn acquire_binding(&self, slot: u32) {
        if slot != NO_ROOM_BUS_SLOT {
            if let Some(count) = self.binding_counts.get(slot as usize) {
                count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn release_binding(&self, slot: u32) {
        if slot != NO_ROOM_BUS_SLOT {
            if let Some(count) = self.binding_counts.get(slot as usize) {
                let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    Some(value.saturating_sub(1))
                });
            }
        }
    }

    fn binding_count(&self, slot: usize) -> u32 {
        self.binding_counts
            .get(slot)
            .map(|count| count.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

#[derive(Clone)]
struct RoomLateParamsControl {
    decay_bits: Arc<AtomicU32>,
    damping_bits: Arc<AtomicU32>,
    diffusion_bits: Arc<AtomicU32>,
}

impl RoomLateParamsControl {
    fn new(preset: AudioReverbPreset) -> Self {
        let preset = preset.sanitized();
        Self {
            decay_bits: Arc::new(AtomicU32::new(preset.decay_seconds.to_bits())),
            damping_bits: Arc::new(AtomicU32::new(preset.damping.to_bits())),
            diffusion_bits: Arc::new(AtomicU32::new(preset.diffusion.to_bits())),
        }
    }

    fn set(&self, preset: AudioReverbPreset) {
        let preset = preset.sanitized();
        self.decay_bits
            .store(preset.decay_seconds.to_bits(), Ordering::Relaxed);
        self.damping_bits
            .store(preset.damping.to_bits(), Ordering::Relaxed);
        self.diffusion_bits
            .store(preset.diffusion.to_bits(), Ordering::Relaxed);
    }

    fn snapshot(&self) -> ReverbSendSnapshot {
        ReverbSendSnapshot {
            gain: 1.0,
            early_reflections: AudioEarlyReflectionField::empty(),
            early_reflections_gain: 0.0,
            early_reflections_high_frequency_gain: 1.0,
            early_reflection_direction: [0.0; 3],
            pre_delay_ms: 0.0,
            early_reflections_spread_ms: 0.0,
            decay_seconds: f32::from_bits(self.decay_bits.load(Ordering::Relaxed))
                .clamp(0.05, 20.0),
            damping: f32::from_bits(self.damping_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
            diffusion: f32::from_bits(self.diffusion_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
        }
    }
}

/// Infinite room output source. It owns the only FDN for this room slot. Source-specific early
/// arrivals never enter this processor; they remain in the physical voice renderer.
struct SharedRoomLateBusSource {
    ingress: Arc<LateBusIngress>,
    params: RoomLateParamsControl,
    tank: ReverbTank,
    generation: u32,
    pending_right: Option<f32>,
    sample_rate: SampleRate,
}

impl SharedRoomLateBusSource {
    fn new(
        ingress: Arc<LateBusIngress>,
        params: RoomLateParamsControl,
        sample_rate: SampleRate,
    ) -> Self {
        let stereo = ChannelCount::new(2).expect("stereo channel count");
        let generation = ingress.generation();
        Self {
            ingress,
            params,
            tank: ReverbTank::new(sample_rate, stereo),
            generation,
            pending_right: None,
            sample_rate,
        }
    }
}

impl Iterator for SharedRoomLateBusSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ingress.generation() != self.generation {
            return None;
        }
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        let input = self.ingress.take_current_frame();
        let params = self.params.snapshot();
        let left = self.tank.process_components(input, params).late;
        let right = self.tank.process_components(input, params).late;
        self.ingress.advance_frame();
        self.pending_right = Some(right);
        Some(left)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for SharedRoomLateBusSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("stereo channel count")
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }

    fn try_seek(&mut self, _: Duration) -> Result<(), SeekError> {
        Err(SeekError::NotSupported {
            underlying_source: std::any::type_name::<Self>(),
        })
    }
}

struct ActiveRoomLateBus {
    room_id: u64,
    _render: BlockVoiceHandle,
    params: RoomLateParamsControl,
}

struct SharedRoomLateBusManager {
    registry: Arc<RoomBusIngressRegistry>,
    slots: Vec<Option<ActiveRoomLateBus>>,
}

impl SharedRoomLateBusManager {
    fn new() -> Self {
        Self {
            registry: Arc::new(RoomBusIngressRegistry::new()),
            slots: std::iter::repeat_with(|| None)
                .take(MAX_SHARED_ROOM_BUSES)
                .collect(),
        }
    }

    #[inline]
    fn registry(&self) -> Arc<RoomBusIngressRegistry> {
        Arc::clone(&self.registry)
    }

    #[inline]
    fn active_bus_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    fn ensure_send(
        &mut self,
        graph: &NativeBlockRenderGraphHandle,
        sample_rate: SampleRate,
        send: AudioReverbSend,
    ) -> Option<u32> {
        let send = send.sanitized();
        if send.room_bus_id == 0 {
            return None;
        }
        if let Some((index, active)) =
            self.slots.iter_mut().enumerate().find_map(|(index, slot)| {
                slot.as_mut()
                    .filter(|active| active.room_id == send.room_bus_id)
                    .map(|active| (index, active))
            })
        {
            active.params.set(send.preset);
            return Some(index as u32);
        }
        let index = self.slots.iter().position(Option::is_none).or_else(|| {
            self.slots.iter().enumerate().find_map(|(index, slot)| {
                (slot.is_some() && self.registry.binding_count(index) == 0).then_some(index)
            })
        })?;
        if let Some(retired) = self.slots[index].take() {
            retired._render.stop();
            self.registry.slots[index].reset_for_reuse();
        }
        let ingress = Arc::clone(&self.registry.slots[index]);
        let params = RoomLateParamsControl::new(send.preset);
        let source = SharedRoomLateBusSource::new(ingress, params.clone(), sample_rate);
        let render = graph
            .add_source(source, 1.0, 1.0, false, Duration::ZERO)
            .ok()?;
        self.slots[index] = Some(ActiveRoomLateBus {
            room_id: send.room_bus_id,
            _render: render,
            params,
        });
        Some(index as u32)
    }

    fn slots_for_environment(
        &mut self,
        graph: &NativeBlockRenderGraphHandle,
        sample_rate: SampleRate,
        environment: AudioEnvironmentState,
    ) -> Option<(u32, u32)> {
        let environment = environment.sanitized();
        let source_requested = environment.source_send.room_bus_id != 0;
        let listener_requested = environment.listener_send.room_bus_id != 0;
        if !source_requested && !listener_requested {
            return None;
        }
        let source = source_requested
            .then(|| self.ensure_send(graph, sample_rate, environment.source_send))
            .flatten();
        let listener = listener_requested
            .then(|| self.ensure_send(graph, sample_rate, environment.listener_send))
            .flatten();
        if (source_requested && source.is_none()) || (listener_requested && listener.is_none()) {
            return None;
        }
        Some((
            source.unwrap_or(NO_ROOM_BUS_SLOT),
            listener.unwrap_or(NO_ROOM_BUS_SLOT),
        ))
    }
}

#[derive(Clone)]
struct RoomBusVoiceBinding {
    inner: Arc<RoomBusVoiceBindingInner>,
}

struct RoomBusVoiceBindingInner {
    registry: Arc<RoomBusIngressRegistry>,
    source_slot: AtomicU32,
    listener_slot: AtomicU32,
    voice_gain_bits: AtomicU32,
}

impl Drop for RoomBusVoiceBindingInner {
    fn drop(&mut self) {
        self.registry
            .release_binding(self.source_slot.load(Ordering::Relaxed));
        self.registry
            .release_binding(self.listener_slot.load(Ordering::Relaxed));
    }
}

impl RoomBusVoiceBinding {
    fn new(
        registry: Arc<RoomBusIngressRegistry>,
        source_slot: u32,
        listener_slot: u32,
        voice_gain: f32,
    ) -> Self {
        registry.acquire_binding(source_slot);
        registry.acquire_binding(listener_slot);
        Self {
            inner: Arc::new(RoomBusVoiceBindingInner {
                registry,
                source_slot: AtomicU32::new(source_slot),
                listener_slot: AtomicU32::new(listener_slot),
                voice_gain_bits: AtomicU32::new(voice_gain.clamp(0.0, 16.0).to_bits()),
            }),
        }
    }

    fn set_slots(&self, source_slot: u32, listener_slot: u32) {
        self.set_slot(&self.inner.source_slot, source_slot);
        self.set_slot(&self.inner.listener_slot, listener_slot);
    }

    fn set_slot(&self, storage: &AtomicU32, slot: u32) {
        let previous = storage.swap(slot, Ordering::AcqRel);
        if previous != slot {
            self.inner.registry.release_binding(previous);
            self.inner.registry.acquire_binding(slot);
        }
    }

    fn set_voice_gain(&self, value: f32) {
        let value = if value.is_finite() {
            value.clamp(0.0, 16.0)
        } else {
            1.0
        };
        self.inner
            .voice_gain_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn voice_gain(&self) -> f32 {
        f32::from_bits(self.inner.voice_gain_bits.load(Ordering::Relaxed)).clamp(0.0, 16.0)
    }

    fn inject(&self, dry_sample: f32, source_gain: f32, listener_gain: f32, channels: usize) {
        let channel_scale = 1.0 / channels.max(1) as f32;
        let base = dry_sample * self.voice_gain() * channel_scale;
        self.inject_slot(
            self.inner.source_slot.load(Ordering::Acquire),
            base * source_gain.clamp(0.0, 2.0),
        );
        self.inject_slot(
            self.inner.listener_slot.load(Ordering::Acquire),
            base * listener_gain.clamp(0.0, 2.0),
        );
    }

    #[inline]
    fn inject_slot(&self, slot: u32, sample: f32) {
        if slot == NO_ROOM_BUS_SLOT {
            return;
        }
        if let Some(ingress) = self.inner.registry.get(slot) {
            ingress.push(sample);
        }
    }
}

impl AudioRuntimeState {
    fn room_bus_binding_for_environment(
        &mut self,
        environment: AudioEnvironmentState,
        voice_gain: f32,
    ) -> Option<RoomBusVoiceBinding> {
        let graph = self.render_graph.clone()?;
        let sample_rate = graph.sample_rate();
        let slots = self
            .room_buses
            .slots_for_environment(&graph, sample_rate, environment)?;
        Some(RoomBusVoiceBinding::new(
            self.room_buses.registry(),
            slots.0,
            slots.1,
            voice_gain,
        ))
    }

    fn rebind_room_bus_voice(
        &mut self,
        binding: &RoomBusVoiceBinding,
        environment: AudioEnvironmentState,
    ) -> bool {
        let graph = match self.render_graph.clone() {
            Some(graph) => graph,
            None => return false,
        };
        let sample_rate = graph.sample_rate();
        let Some((source, listener)) =
            self.room_buses
                .slots_for_environment(&graph, sample_rate, environment)
        else {
            binding.set_slots(NO_ROOM_BUS_SLOT, NO_ROOM_BUS_SLOT);
            // Dry/no-room transitions need no late processor. A wet legacy send with id 0 must
            // rematerialize onto the per-voice compatibility FDN rather than silently losing tail.
            return environment.source_send.room_bus_id == 0
                && environment.listener_send.room_bus_id == 0
                && !environment.is_wet();
        };
        binding.set_slots(source, listener);
        true
    }
}

#[cfg(test)]
mod room_bus_tests {
    use super::*;

    #[test]
    fn ingress_accumulates_multiple_voice_sends_into_one_future_room_frame() {
        let ingress = LateBusIngress::new();
        ingress.push(0.25);
        ingress.push(0.75);
        assert_eq!(ingress.take_current_frame(), 0.0);
        ingress.advance_frame();
        assert_eq!(ingress.take_current_frame(), 0.0);
        ingress.advance_frame();
        assert!((ingress.take_current_frame() - 1.0).abs() < 1.0e-6);
        assert_eq!(ingress.take_current_frame(), 0.0);
    }

    #[test]
    fn two_voice_bindings_share_one_ingress_without_per_voice_late_state() {
        let registry = Arc::new(RoomBusIngressRegistry::new());
        let a = RoomBusVoiceBinding::new(Arc::clone(&registry), 3, NO_ROOM_BUS_SLOT, 0.5);
        let b = RoomBusVoiceBinding::new(Arc::clone(&registry), 3, NO_ROOM_BUS_SLOT, 0.25);
        a.inject(1.0, 1.0, 0.0, 1);
        b.inject(1.0, 1.0, 0.0, 1);
        let ingress = &registry.slots[3];
        ingress.advance_frame();
        ingress.advance_frame();
        assert!((ingress.take_current_frame() - 0.75).abs() < 1.0e-6);
    }

    #[test]
    fn per_voice_early_only_tank_never_produces_fdn_late_energy() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(1).expect("mono");
        let mut tank = ReverbTank::new_early_only(sample_rate, channels);
        let params = ReverbSendSnapshot::from_send(AudioReverbSend {
            room_bus_id: 17,
            gain: 1.0,
            preset: AudioReverbPreset::room(),
            early_reflections: AudioEarlyReflectionField::empty(),
            early_reflection_direction: [0.0; 3],
        });
        for frame in 0..8_000usize {
            let components = tank.process_components(if frame == 0 { 1.0 } else { 0.0 }, params);
            assert_eq!(components.late, 0.0);
        }
    }

    #[test]
    fn one_shared_room_source_keeps_late_tail_after_voice_injection_stops() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let ingress = Arc::new(LateBusIngress::new());
        let params = RoomLateParamsControl::new(AudioReverbPreset::room());
        let mut source = SharedRoomLateBusSource::new(Arc::clone(&ingress), params, sample_rate);
        ingress.push(1.0);
        let mut late_energy = 0.0_f32;
        for _ in 0..12_000usize {
            late_energy += source.next().unwrap_or(0.0).abs();
        }
        assert!(
            late_energy > 0.01,
            "shared room FDN must retain a late tail"
        );
    }

    #[test]
    fn binding_refcounts_follow_live_room_rebind_and_last_clone_drop() {
        let registry = Arc::new(RoomBusIngressRegistry::new());
        let binding = RoomBusVoiceBinding::new(Arc::clone(&registry), 1, NO_ROOM_BUS_SLOT, 1.0);
        assert_eq!(registry.binding_count(1), 1);
        let clone = binding.clone();
        drop(binding);
        assert_eq!(registry.binding_count(1), 1);
        clone.set_slots(2, NO_ROOM_BUS_SLOT);
        assert_eq!(registry.binding_count(1), 0);
        assert_eq!(registry.binding_count(2), 1);
        drop(clone);
        assert_eq!(registry.binding_count(2), 0);
    }

    #[test]
    fn ingress_generation_invalidates_retired_room_source_before_slot_reuse() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let ingress = Arc::new(LateBusIngress::new());
        let params = RoomLateParamsControl::new(AudioReverbPreset::room());
        let mut retired =
            SharedRoomLateBusSource::new(Arc::clone(&ingress), params.clone(), sample_rate);
        let old_generation = ingress.generation();
        let new_generation = ingress.reset_for_reuse();
        assert_ne!(old_generation, new_generation);
        assert!(retired.next().is_none());
        let mut replacement = SharedRoomLateBusSource::new(ingress, params, sample_rate);
        assert!(replacement.next().is_some());
    }
}
