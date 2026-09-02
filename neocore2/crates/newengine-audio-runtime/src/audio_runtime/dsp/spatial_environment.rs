#[derive(Clone, Copy, Debug)]
struct SpatialMixSnapshot {
    emitter_position: [f32; 3],
    left_ear: [f32; 3],
    right_ear: [f32; 3],
}

#[derive(Clone, Debug)]
struct SpatialMixControl {
    emitter_bits: [Arc<AtomicU32>; 3],
    left_ear_bits: [Arc<AtomicU32>; 3],
    right_ear_bits: [Arc<AtomicU32>; 3],
}

impl SpatialMixControl {
    fn new(emitter_position: [f32; 3], left_ear: [f32; 3], right_ear: [f32; 3]) -> Self {
        Self {
            emitter_bits: emitter_position.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
            left_ear_bits: left_ear.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
            right_ear_bits: right_ear.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
        }
    }

    fn set_emitter_position(&self, value: [f32; 3]) {
        store_atomic_vec3(&self.emitter_bits, value);
    }

    fn set_ears(&self, left: [f32; 3], right: [f32; 3]) {
        store_atomic_vec3(&self.left_ear_bits, left);
        store_atomic_vec3(&self.right_ear_bits, right);
    }

    fn snapshot(&self) -> SpatialMixSnapshot {
        SpatialMixSnapshot {
            emitter_position: load_atomic_vec3(&self.emitter_bits),
            left_ear: load_atomic_vec3(&self.left_ear_bits),
            right_ear: load_atomic_vec3(&self.right_ear_bits),
        }
    }
}

#[inline]
fn store_atomic_vec3(bits: &[Arc<AtomicU32>; 3], value: [f32; 3]) {
    for (slot, value) in bits.iter().zip(value) {
        slot.store(value.to_bits(), Ordering::Relaxed);
    }
}

#[inline]
fn load_atomic_vec3(bits: &[Arc<AtomicU32>; 3]) -> [f32; 3] {
    std::array::from_fn(|index| f32::from_bits(bits[index].load(Ordering::Relaxed)))
}

/// Direction-only speaker pan for the direct field. Distance energy is deliberately absent here:
/// authored `AudioAttenuationSettings` is evaluated once in `voice_output_gain`/materialization.
/// Applying another inverse-distance law in the spatializer caused spatial voices to be attenuated
/// twice and could make otherwise healthy physical voices effectively inaudible.
fn direct_stereo_gains(spatial: SpatialMixSnapshot) -> [f32; 2] {
    let listener_center = [
        (spatial.left_ear[0] + spatial.right_ear[0]) * 0.5,
        (spatial.left_ear[1] + spatial.right_ear[1]) * 0.5,
        (spatial.left_ear[2] + spatial.right_ear[2]) * 0.5,
    ];
    let listener_to_emitter = [
        spatial.emitter_position[0] - listener_center[0],
        spatial.emitter_position[1] - listener_center[1],
        spatial.emitter_position[2] - listener_center[2],
    ];
    reflection_stereo_gains(listener_to_emitter, spatial)
}

/// Equal-power speaker pan from a world-space arrival vector. Ear separation defines listener
/// right; zero/unknown direction remains centered. This is intentional speaker spatialization,
/// not an HRTF/binaural claim.
fn reflection_stereo_gains(direction: [f32; 3], spatial: SpatialMixSnapshot) -> [f32; 2] {
    let right = [
        spatial.right_ear[0] - spatial.left_ear[0],
        spatial.right_ear[1] - spatial.left_ear[1],
        spatial.right_ear[2] - spatial.left_ear[2],
    ];
    let right_len = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2]).sqrt();
    let dir_len =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    if right_len <= 1.0e-5 || dir_len <= 1.0e-5 || !right_len.is_finite() || !dir_len.is_finite() {
        return [1.0, 1.0];
    }
    let pan = ((direction[0] * right[0] + direction[1] * right[1] + direction[2] * right[2])
        / (dir_len * right_len))
        .clamp(-1.0, 1.0);
    [(1.0 - pan).sqrt(), (1.0 + pan).sqrt()]
}

fn spatialize_early_components(
    left: ReverbComponents,
    right: ReverbComponents,
    params: ReverbSendSnapshot,
    spatial: SpatialMixSnapshot,
) -> [f32; 2] {
    if params.early_reflections.is_empty() {
        let gains = reflection_stereo_gains(params.early_reflection_direction, spatial);
        return [left.early * gains[0], right.early * gains[1]];
    }
    let mut output = [0.0_f32; 2];
    for (index, tap) in params.early_reflections.active().iter().enumerate() {
        let gains = reflection_stereo_gains(tap.direction, spatial);
        output[0] += left.early_taps[index] * gains[0];
        output[1] += right.early_taps[index] * gains[1];
    }
    output
}
/// Spatial voice renderer that keeps one decode/timeline while giving direct, early and late
/// acoustic fields independent spatial laws. Input must be mono; output is interleaved stereo.
struct DynamicSpatialEnvironmentSource<I> {
    input: I,
    environment_control: EnvironmentFilterControl,
    spatial_control: SpatialMixControl,
    source_tank: ReverbTank,
    listener_tank: ReverbTank,
    direct_path: DirectPathProcessor,
    source_params: ReverbSendSnapshot,
    listener_params: ReverbSendSnapshot,
    direct_params: DirectPathSnapshot,
    spatial: SpatialMixSnapshot,
    late_binding: Option<RoomBusVoiceBinding>,
    pending_right: Option<f32>,
    control_countdown: u8,
}

impl<I> DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[cfg(test)]
    fn new(
        input: I,
        environment_control: EnvironmentFilterControl,
        spatial_control: SpatialMixControl,
    ) -> Self {
        Self::new_with_late_binding(input, environment_control, spatial_control, None)
    }

    fn new_with_late_binding(
        input: I,
        environment_control: EnvironmentFilterControl,
        spatial_control: SpatialMixControl,
        late_binding: Option<RoomBusVoiceBinding>,
    ) -> Self {
        debug_assert_eq!(input.channels().get(), 1);
        let sample_rate = input.sample_rate();
        let stereo = ChannelCount::new(2).expect("stereo channel count");
        let mono = ChannelCount::new(1).expect("mono channel count");
        let source_params = environment_control.source.snapshot();
        let listener_params = environment_control.listener.snapshot();
        let direct_params = environment_control.direct.snapshot();
        let spatial = spatial_control.snapshot();
        let per_voice_late = late_binding.is_none()
            && (source_params.gain > 1.0e-4 || listener_params.gain > 1.0e-4);
        Self {
            input,
            environment_control,
            spatial_control,
            source_tank: if per_voice_late {
                ReverbTank::new(sample_rate, stereo)
            } else {
                ReverbTank::new_early_only(sample_rate, stereo)
            },
            listener_tank: if per_voice_late {
                ReverbTank::new(sample_rate, stereo)
            } else {
                ReverbTank::new_early_only(sample_rate, stereo)
            },
            direct_path: DirectPathProcessor::new(sample_rate, mono),
            source_params,
            listener_params,
            direct_params,
            spatial,
            late_binding,
            pending_right: None,
            control_countdown: 0,
        }
    }

    fn refresh_controls(&mut self) {
        if self.control_countdown == 0 {
            self.source_params = self.environment_control.source.snapshot();
            self.listener_params = self.environment_control.listener.snapshot();
            self.direct_params = self.environment_control.direct.snapshot();
            self.spatial = self.spatial_control.snapshot();
            self.control_countdown = 63;
        } else {
            self.control_countdown -= 1;
        }
    }

    fn reset_state(&mut self) {
        self.source_tank.reset();
        self.listener_tank.reset();
        self.direct_path.reset();
        self.pending_right = None;
        self.control_countdown = 0;
    }
}

impl<I> Iterator for DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        let dry = self.input.next()?;
        self.refresh_controls();
        let [source_gain, listener_gain] =
            normalized_reverb_send_gains(self.source_params.gain, self.listener_params.gain);
        if let Some(binding) = self.late_binding.as_ref() {
            binding.inject(dry, source_gain, listener_gain, 1);
        }
        let direct = self.direct_path.process(dry, self.direct_params);
        // Tanks are stereo internally: feed the same mono frame twice so channel-dependent early
        // offsets and signed FDN rows create a decorrelated diffuse field without a second decode.
        let source_left = self.source_tank.process_components(dry, self.source_params);
        let source_right = self.source_tank.process_components(dry, self.source_params);
        let listener_left = self
            .listener_tank
            .process_components(dry, self.listener_params);
        let listener_right = self
            .listener_tank
            .process_components(dry, self.listener_params);

        let direct_gain = direct_stereo_gains(self.spatial);
        let source_early = spatialize_early_components(
            source_left,
            source_right,
            self.source_params,
            self.spatial,
        );
        let listener_early = spatialize_early_components(
            listener_left,
            listener_right,
            self.listener_params,
            self.spatial,
        );
        let left = direct * direct_gain[0]
            + source_early[0] * source_gain
            + listener_early[0] * listener_gain
            + source_left.late * source_gain
            + listener_left.late * listener_gain;
        let right = direct * direct_gain[1]
            + source_early[1] * source_gain
            + listener_early[1] * listener_gain
            + source_right.late * source_gain
            + listener_right.late * listener_gain;
        self.pending_right = Some(right);
        Some(left)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<I> Source for DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("stereo channel count")
    }

    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_state();
        Ok(())
    }
}

/// Two independent dynamic room sends are rendered from the same dry source. The
/// control values are atomically replaceable; delay/feedback history remains attached
/// to the physical voice and is reset only by an actual source seek.
struct DynamicEnvironmentSource<I> {
    input: I,
    control: EnvironmentFilterControl,
    source_tank: ReverbTank,
    listener_tank: ReverbTank,
    direct_path: DirectPathProcessor,
    source_params: ReverbSendSnapshot,
    listener_params: ReverbSendSnapshot,
    direct_params: DirectPathSnapshot,
    late_binding: Option<RoomBusVoiceBinding>,
    control_countdown: u8,
}

impl<I> DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[cfg(test)]
    fn new(input: I, control: EnvironmentFilterControl) -> Self {
        Self::new_with_late_binding(input, control, None)
    }

    fn new_with_late_binding(
        input: I,
        control: EnvironmentFilterControl,
        late_binding: Option<RoomBusVoiceBinding>,
    ) -> Self {
        let sample_rate = input.sample_rate();
        let channels = input.channels();
        let source_params = control.source.snapshot();
        let listener_params = control.listener.snapshot();
        let direct_params = control.direct.snapshot();
        let per_voice_late = late_binding.is_none()
            && (source_params.gain > 1.0e-4 || listener_params.gain > 1.0e-4);
        Self {
            input,
            control,
            source_tank: if per_voice_late {
                ReverbTank::new(sample_rate, channels)
            } else {
                ReverbTank::new_early_only(sample_rate, channels)
            },
            listener_tank: if per_voice_late {
                ReverbTank::new(sample_rate, channels)
            } else {
                ReverbTank::new_early_only(sample_rate, channels)
            },
            direct_path: DirectPathProcessor::new(sample_rate, channels),
            source_params,
            listener_params,
            direct_params,
            late_binding,
            control_countdown: 0,
        }
    }

    fn refresh_controls(&mut self) {
        if self.control_countdown == 0 {
            self.source_params = self.control.source.snapshot();
            self.listener_params = self.control.listener.snapshot();
            self.direct_params = self.control.direct.snapshot();
            self.control_countdown = 63;
        } else {
            self.control_countdown -= 1;
        }
    }

    fn reset_environment_state(&mut self) {
        self.source_tank.reset();
        self.listener_tank.reset();
        self.direct_path.reset();
        self.control_countdown = 0;
    }
}

impl<I> Iterator for DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let dry = self.input.next()?;
        self.refresh_controls();
        let [source_gain, listener_gain] =
            normalized_reverb_send_gains(self.source_params.gain, self.listener_params.gain);
        if let Some(binding) = self.late_binding.as_ref() {
            binding.inject(
                dry,
                source_gain,
                listener_gain,
                usize::from(self.input.channels().get()),
            );
        }
        let direct = self.direct_path.process(dry, self.direct_params);
        let source_wet = self.source_tank.process(dry, self.source_params) * source_gain;
        let listener_wet = self.listener_tank.process(dry, self.listener_params) * listener_gain;
        // The direct alternate path owns portal/diffraction delay and spectral loss. Reverb
        // sends remain independent indirect energy and preserve their own room history.
        Some(direct + source_wet + listener_wet)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> Source for DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_environment_state();
        Ok(())
    }
}
