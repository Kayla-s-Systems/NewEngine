use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use rodio::source::{SeekError, Source, UniformSourceIterator};
use rodio::Sample;
use rodio::{ChannelCount, SampleRate};

#[path = "block_render/commands.rs"]
mod commands;

use commands::{render_command_node_id, RenderCommand, RenderCommandKind};

pub(crate) const NATIVE_BLOCK_FRAMES: usize = 256;
const MAX_BLOCK_NODES: usize = 256;
const MAX_RENDER_COMMANDS: usize = 4_096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativeBlockRenderStats {
    pub output_sample: u64,
    pub rendered_blocks: u64,
    pub rendered_frames: u64,
    pub split_segments: u64,
    pub applied_commands: u64,
    pub dropped_commands: u64,
    pub active_nodes: usize,
}

struct SharedRenderStats {
    output_sample: AtomicU64,
    rendered_blocks: AtomicU64,
    rendered_frames: AtomicU64,
    split_segments: AtomicU64,
    applied_commands: AtomicU64,
    dropped_commands: AtomicU64,
    active_nodes: AtomicU64,
}

impl Default for SharedRenderStats {
    fn default() -> Self {
        Self {
            output_sample: AtomicU64::new(0),
            rendered_blocks: AtomicU64::new(0),
            rendered_frames: AtomicU64::new(0),
            split_segments: AtomicU64::new(0),
            applied_commands: AtomicU64::new(0),
            dropped_commands: AtomicU64::new(0),
            active_nodes: AtomicU64::new(0),
        }
    }
}

impl SharedRenderStats {
    fn snapshot(&self) -> NativeBlockRenderStats {
        NativeBlockRenderStats {
            output_sample: self.output_sample.load(Ordering::Acquire),
            rendered_blocks: self.rendered_blocks.load(Ordering::Relaxed),
            rendered_frames: self.rendered_frames.load(Ordering::Relaxed),
            split_segments: self.split_segments.load(Ordering::Relaxed),
            applied_commands: self.applied_commands.load(Ordering::Relaxed),
            dropped_commands: self.dropped_commands.load(Ordering::Relaxed),
            active_nodes: self.active_nodes.load(Ordering::Relaxed) as usize,
        }
    }
}

struct SharedVoiceState {
    source_position_ns: AtomicU64,
    finished: AtomicBool,
    cancelled: AtomicBool,
}

impl Default for SharedVoiceState {
    fn default() -> Self {
        Self {
            source_position_ns: AtomicU64::new(0),
            finished: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        }
    }
}

#[derive(Clone)]
pub(crate) struct BlockVoiceHandle {
    node_id: u64,
    command_tx: Sender<RenderCommand>,
    next_sequence: Arc<AtomicU64>,
    stats: Arc<SharedRenderStats>,
    state: Arc<SharedVoiceState>,
}

impl BlockVoiceHandle {
    #[inline]
    pub(crate) fn get_pos(&self) -> Duration {
        Duration::from_nanos(self.state.source_position_ns.load(Ordering::Acquire))
    }

    #[inline]
    pub(crate) fn empty(&self) -> bool {
        self.state.finished.load(Ordering::Acquire)
    }

    pub(crate) fn set_volume(&self, gain: f32) {
        let gain = finite_gain(gain);
        let _ = self.submit_now(RenderCommandKind::SetGain {
            node_id: self.node_id,
            gain,
        });
    }

    pub(crate) fn set_speed(&self, speed: f32) {
        let speed = finite_speed(speed);
        let _ = self.submit_now(RenderCommandKind::SetSpeed {
            node_id: self.node_id,
            speed,
        });
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        let _ = self.submit_now(RenderCommandKind::SetPaused {
            node_id: self.node_id,
            paused,
        });
    }

    pub(crate) fn stop(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        self.state.finished.store(true, Ordering::Release);
        let _ = self.submit_now(RenderCommandKind::Remove {
            node_id: self.node_id,
        });
    }

    pub(crate) fn try_seek(&self, position: Duration) -> Result<(), String> {
        self.submit_now(RenderCommandKind::Seek {
            node_id: self.node_id,
            position,
        })
    }

    pub(crate) fn schedule_gain_at(
        &self,
        sample: u64,
        target: f32,
        duration_samples: u64,
        schedule_id: u64,
    ) -> Result<(), String> {
        self.submit(
            Some(sample),
            Some(schedule_id),
            RenderCommandKind::RampGain {
                node_id: self.node_id,
                target: finite_gain(target),
                duration_samples,
            },
        )
    }

    pub(crate) fn schedule_stop_at(&self, sample: u64, schedule_id: u64) -> Result<(), String> {
        self.submit(
            Some(sample),
            Some(schedule_id),
            RenderCommandKind::Remove {
                node_id: self.node_id,
            },
        )
    }

    pub(crate) fn cancel_scheduled(&self, schedule_id: u64) -> Result<(), String> {
        self.submit_now(RenderCommandKind::CancelScheduled {
            node_id: self.node_id,
            schedule_id,
        })
    }

    fn submit_now(&self, kind: RenderCommandKind) -> Result<(), String> {
        self.submit(None, None, kind)
    }

    fn submit(
        &self,
        at_sample: Option<u64>,
        schedule_id: Option<u64>,
        kind: RenderCommandKind,
    ) -> Result<(), String> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed).max(1);
        let command = RenderCommand {
            at_sample,
            sequence,
            schedule_id,
            kind,
        };
        match self.command_tx.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                Err("native block render command queue is full".to_owned())
            }
            Err(TrySendError::Disconnected(_)) => {
                Err("native block render command queue disconnected".to_owned())
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct NativeBlockRenderGraphHandle {
    channels: ChannelCount,
    sample_rate: SampleRate,
    command_tx: Sender<RenderCommand>,
    next_node_id: Arc<AtomicU64>,
    next_sequence: Arc<AtomicU64>,
    stats: Arc<SharedRenderStats>,
}

impl NativeBlockRenderGraphHandle {
    #[inline]
    pub(crate) fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[inline]
    pub(crate) fn output_sample(&self) -> u64 {
        self.stats.output_sample.load(Ordering::Acquire)
    }

    #[inline]
    pub(crate) fn stats(&self) -> NativeBlockRenderStats {
        self.stats.snapshot()
    }

    pub(crate) fn add_source<S>(
        &self,
        source: S,
        gain: f32,
        speed: f32,
        paused: bool,
        source_position: Duration,
    ) -> Result<BlockVoiceHandle, String>
    where
        S: Source + Send + 'static,
    {
        self.add_boxed_source(Box::new(source), gain, speed, paused, source_position, None)
    }

    pub(crate) fn add_boxed_source(
        &self,
        source: Box<dyn Source + Send>,
        gain: f32,
        speed: f32,
        paused: bool,
        source_position: Duration,
        at_sample: Option<u64>,
    ) -> Result<BlockVoiceHandle, String> {
        let node_id = self.next_node_id.fetch_add(1, Ordering::Relaxed).max(1);
        let state = Arc::new(SharedVoiceState::default());
        state.source_position_ns.store(
            source_position.as_nanos().min(u128::from(u64::MAX)) as u64,
            Ordering::Release,
        );
        let uniform = UniformSourceIterator::new(source, self.channels, self.sample_rate);
        let adapter = BlockSourceAdapter::new(Box::new(uniform), self.channels);
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed).max(1);
        let command = RenderCommand {
            at_sample,
            sequence,
            schedule_id: None,
            kind: RenderCommandKind::Add {
                node: BlockVoiceNode::new(
                    node_id,
                    adapter,
                    finite_gain(gain),
                    finite_speed(speed),
                    paused,
                    source_position,
                    Arc::clone(&state),
                    self.sample_rate,
                    self.channels,
                ),
            },
        };
        match self.command_tx.try_send(command) {
            Ok(()) => Ok(BlockVoiceHandle {
                node_id,
                command_tx: self.command_tx.clone(),
                next_sequence: Arc::clone(&self.next_sequence),
                stats: Arc::clone(&self.stats),
                state,
            }),
            Err(TrySendError::Full(_)) => {
                self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                Err("native block render command queue is full".to_owned())
            }
            Err(TrySendError::Disconnected(_)) => {
                Err("native block render command queue disconnected".to_owned())
            }
        }
    }
}

pub(crate) fn native_block_render_graph(
    channels: ChannelCount,
    sample_rate: SampleRate,
) -> (NativeBlockRenderGraphHandle, NativeBlockRenderSource) {
    let (command_tx, command_rx) = bounded(MAX_RENDER_COMMANDS);
    let stats = Arc::new(SharedRenderStats::default());
    let handle = NativeBlockRenderGraphHandle {
        channels,
        sample_rate,
        command_tx,
        next_node_id: Arc::new(AtomicU64::new(1)),
        next_sequence: Arc::new(AtomicU64::new(1)),
        stats: Arc::clone(&stats),
    };
    let source = NativeBlockRenderSource::new(channels, sample_rate, command_rx, stats);
    (handle, source)
}

struct GainRamp {
    from: f32,
    target: f32,
    start_sample: u64,
    end_sample: u64,
}

struct BlockVoiceNode {
    id: u64,
    source: BlockSourceAdapter,
    gain: f32,
    speed: f32,
    paused: bool,
    output_position_seconds: f64,
    state: Arc<SharedVoiceState>,
    sample_rate: SampleRate,
    scratch: Vec<Sample>,
    gain_ramp: Option<GainRamp>,
}

impl BlockVoiceNode {
    fn new(
        id: u64,
        source: BlockSourceAdapter,
        gain: f32,
        speed: f32,
        paused: bool,
        source_position: Duration,
        state: Arc<SharedVoiceState>,
        sample_rate: SampleRate,
        channels: ChannelCount,
    ) -> Self {
        Self {
            id,
            source,
            gain,
            speed,
            paused,
            output_position_seconds: source_position.as_secs_f64(),
            state,
            sample_rate,
            scratch: vec![0.0; usize::from(channels.get())],
            gain_ramp: None,
        }
    }

    fn set_gain(&mut self, gain: f32) {
        self.gain = finite_gain(gain);
        self.gain_ramp = None;
    }

    fn ramp_gain(&mut self, target: f32, start_sample: u64, duration_samples: u64) {
        let target = finite_gain(target);
        if duration_samples == 0 {
            self.set_gain(target);
            return;
        }
        self.gain_ramp = Some(GainRamp {
            from: self.gain_at(start_sample),
            target,
            start_sample,
            end_sample: start_sample.saturating_add(duration_samples),
        });
    }

    fn gain_at(&mut self, sample: u64) -> f32 {
        let Some(ramp) = self.gain_ramp.as_ref() else {
            return self.gain;
        };
        if sample <= ramp.start_sample {
            return ramp.from;
        }
        if sample >= ramp.end_sample {
            self.gain = ramp.target;
            self.gain_ramp = None;
            return self.gain;
        }
        let elapsed = sample - ramp.start_sample;
        let duration = ramp.end_sample - ramp.start_sample;
        let t = elapsed as f64 / duration.max(1) as f64;
        (f64::from(ramp.from) + (f64::from(ramp.target) - f64::from(ramp.from)) * t) as f32
    }

    fn seek(&mut self, position: Duration) {
        if self.source.try_seek(position).is_ok() {
            self.output_position_seconds = position.as_secs_f64();
            self.publish_position();
            self.state.finished.store(false, Ordering::Release);
        }
    }

    fn render_frame(&mut self, output: &mut [Sample], absolute_sample: u64) -> bool {
        if self.paused {
            return true;
        }
        if !self.source.render_frame(&mut self.scratch, self.speed) {
            self.state.finished.store(true, Ordering::Release);
            return false;
        }
        let gain = self.gain_at(absolute_sample);
        for (dst, src) in output.iter_mut().zip(self.scratch.iter().copied()) {
            let sample = if src.is_finite() { src } else { 0.0 };
            *dst += sample * gain;
        }
        self.output_position_seconds += 1.0 / f64::from(self.sample_rate.get());
        self.publish_position();
        true
    }

    fn publish_position(&self) {
        let nanos = (self.output_position_seconds.max(0.0) * 1_000_000_000.0)
            .round()
            .clamp(0.0, u64::MAX as f64) as u64;
        self.state
            .source_position_ns
            .store(nanos, Ordering::Release);
    }
}

struct BlockSourceAdapter {
    source: Box<dyn Source + Send>,
    channels: usize,
    current: Vec<Sample>,
    next: Vec<Sample>,
    phase: f64,
    primed: bool,
    next_valid: bool,
    exhausted: bool,
}

impl BlockSourceAdapter {
    fn new(source: Box<dyn Source + Send>, channels: ChannelCount) -> Self {
        let channels = usize::from(channels.get());
        Self {
            source,
            channels,
            current: vec![0.0; channels],
            next: vec![0.0; channels],
            phase: 0.0,
            primed: false,
            next_valid: false,
            exhausted: false,
        }
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.source.try_seek(position)?;
        self.phase = 0.0;
        self.primed = false;
        self.next_valid = false;
        self.exhausted = false;
        Ok(())
    }

    fn read_frame_from(source: &mut Box<dyn Source + Send>, frame: &mut [Sample]) -> bool {
        for sample in frame {
            let Some(value) = source.next() else {
                return false;
            };
            *sample = value;
        }
        true
    }

    fn prime(&mut self) -> bool {
        if self.primed {
            return !self.exhausted;
        }
        if !Self::read_frame_from(&mut self.source, &mut self.current) {
            self.exhausted = true;
            return false;
        }
        self.next_valid = Self::read_frame_from(&mut self.source, &mut self.next);
        if !self.next_valid {
            self.next.copy_from_slice(&self.current);
        }
        self.primed = true;
        true
    }

    fn render_frame(&mut self, out: &mut [Sample], speed: f32) -> bool {
        debug_assert_eq!(out.len(), self.channels);
        if self.exhausted || !self.prime() {
            return false;
        }
        let t = self.phase.clamp(0.0, 1.0) as f32;
        for channel in 0..self.channels {
            out[channel] = self.current[channel] + (self.next[channel] - self.current[channel]) * t;
        }

        self.phase += f64::from(finite_speed(speed));
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            if !self.next_valid {
                self.exhausted = true;
                break;
            }
            self.current.copy_from_slice(&self.next);
            self.next_valid = Self::read_frame_from(&mut self.source, &mut self.next);
            if !self.next_valid {
                self.next.copy_from_slice(&self.current);
            }
        }
        true
    }
}

const MASTER_OUTPUT_PEAK: f32 = 1.0;
const MASTER_LIMITER_RELEASE_SECONDS: f32 = 0.080;

#[derive(Clone, Copy, Debug)]
struct OutputPeakLimiter {
    gain: f32,
    release_alpha: f32,
}

impl OutputPeakLimiter {
    fn new(sample_rate: SampleRate) -> Self {
        let frames = (sample_rate.get() as f32 * MASTER_LIMITER_RELEASE_SECONDS).max(1.0);
        Self {
            gain: 1.0,
            release_alpha: 1.0 - (-1.0 / frames).exp(),
        }
    }

    #[inline]
    fn process_frame(&mut self, frame: &mut [Sample]) {
        let peak = frame
            .iter()
            .copied()
            .filter(|sample| sample.is_finite())
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        let target = if peak > MASTER_OUTPUT_PEAK {
            (MASTER_OUTPUT_PEAK / peak).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if target < self.gain {
            // Instant attack prevents the overloaded frame from ever reaching the device.
            self.gain = target;
        } else {
            self.gain += (1.0 - self.gain) * self.release_alpha;
        }
        for sample in frame {
            let finite = if sample.is_finite() { *sample } else { 0.0 };
            *sample = (finite * self.gain).clamp(-MASTER_OUTPUT_PEAK, MASTER_OUTPUT_PEAK);
        }
    }
}

pub(crate) struct NativeBlockRenderSource {
    channels: ChannelCount,
    sample_rate: SampleRate,
    command_rx: Receiver<RenderCommand>,
    stats: Arc<SharedRenderStats>,
    nodes: Vec<Option<BlockVoiceNode>>,
    scheduled: Vec<RenderCommand>,
    block: Vec<Sample>,
    block_cursor: usize,
    output_sample: u64,
    output_limiter: OutputPeakLimiter,
}

impl NativeBlockRenderSource {
    fn new(
        channels: ChannelCount,
        sample_rate: SampleRate,
        command_rx: Receiver<RenderCommand>,
        stats: Arc<SharedRenderStats>,
    ) -> Self {
        let nodes = std::iter::repeat_with(|| None)
            .take(MAX_BLOCK_NODES)
            .collect::<Vec<_>>();
        Self {
            channels,
            sample_rate,
            command_rx,
            stats,
            nodes,
            scheduled: Vec::with_capacity(MAX_RENDER_COMMANDS),
            block: vec![0.0; NATIVE_BLOCK_FRAMES * usize::from(channels.get())],
            block_cursor: NATIVE_BLOCK_FRAMES * usize::from(channels.get()),
            output_sample: 0,
            output_limiter: OutputPeakLimiter::new(sample_rate),
        }
    }

    fn drain_commands(&mut self) {
        let mut inserted = false;
        while let Ok(mut command) = self.command_rx.try_recv() {
            command.at_sample = Some(command.resolved_sample(self.output_sample));
            if self.scheduled.len() >= MAX_RENDER_COMMANDS {
                self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                if let RenderCommandKind::Add { node } = command.kind {
                    node.state.finished.store(true, Ordering::Release);
                }
                continue;
            }
            self.scheduled.push(command);
            inserted = true;
        }
        if inserted {
            // Keep the earliest command at the end so the audio callback consumes due
            // work with O(1) pop() instead of draining the front and shifting the tail.
            self.scheduled
                .sort_unstable_by(|left, right| right.cmp(left));
        }
    }

    fn render_next_block(&mut self) {
        self.drain_commands();
        self.block.fill(0.0);
        let block_start = self.output_sample;
        let block_end = block_start.saturating_add(NATIVE_BLOCK_FRAMES as u64);
        let mut cursor_sample = block_start;
        let mut cursor_frame = 0usize;

        loop {
            self.apply_due_commands(cursor_sample);
            let next_boundary = self
                .scheduled
                .last()
                .and_then(|command| command.at_sample)
                .filter(|sample| *sample < block_end)
                .unwrap_or(block_end)
                .max(cursor_sample);
            if next_boundary > cursor_sample {
                let frames = (next_boundary - cursor_sample) as usize;
                self.render_segment(cursor_frame, frames, cursor_sample);
                cursor_frame += frames;
                cursor_sample = next_boundary;
                if cursor_sample < block_end {
                    self.stats.split_segments.fetch_add(1, Ordering::Relaxed);
                }
            }
            if cursor_sample >= block_end {
                break;
            }
        }

        self.output_sample = block_end;
        self.stats.output_sample.store(block_end, Ordering::Release);
        self.stats.rendered_blocks.fetch_add(1, Ordering::Relaxed);
        self.stats
            .rendered_frames
            .fetch_add(NATIVE_BLOCK_FRAMES as u64, Ordering::Relaxed);
        self.stats.active_nodes.store(
            self.nodes.iter().filter(|node| node.is_some()).count() as u64,
            Ordering::Relaxed,
        );
        self.block_cursor = 0;
    }

    fn apply_due_commands(&mut self, sample: u64) {
        while self
            .scheduled
            .last()
            .is_some_and(|command| command.at_sample.unwrap_or(sample) <= sample)
        {
            let command = self
                .scheduled
                .pop()
                .expect("scheduled command checked by last()");
            self.apply_command(command, sample);
            self.stats.applied_commands.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn apply_command(&mut self, command: RenderCommand, sample: u64) {
        match command.kind {
            RenderCommandKind::Add { node } => {
                if node.state.cancelled.load(Ordering::Acquire) {
                    node.state.finished.store(true, Ordering::Release);
                    return;
                }
                if let Some(slot) = self.nodes.iter_mut().find(|slot| slot.is_none()) {
                    node.state.finished.store(false, Ordering::Release);
                    *slot = Some(node);
                } else {
                    node.state.finished.store(true, Ordering::Release);
                    self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                }
            }
            RenderCommandKind::Remove { node_id } => {
                if let Some(slot) = self.node_slot_mut(node_id) {
                    if let Some(node) = slot.take() {
                        node.state.finished.store(true, Ordering::Release);
                    }
                }
            }
            RenderCommandKind::SetGain { node_id, gain } => {
                if let Some(node) = self.node_mut(node_id) {
                    node.set_gain(gain);
                }
            }
            RenderCommandKind::RampGain {
                node_id,
                target,
                duration_samples,
            } => {
                if let Some(node) = self.node_mut(node_id) {
                    node.ramp_gain(target, sample, duration_samples);
                }
            }
            RenderCommandKind::SetSpeed { node_id, speed } => {
                if let Some(node) = self.node_mut(node_id) {
                    node.speed = finite_speed(speed);
                }
            }
            RenderCommandKind::SetPaused { node_id, paused } => {
                if let Some(node) = self.node_mut(node_id) {
                    node.paused = paused;
                }
            }
            RenderCommandKind::Seek { node_id, position } => {
                if let Some(node) = self.node_mut(node_id) {
                    node.seek(position);
                }
            }
            RenderCommandKind::CancelScheduled {
                node_id,
                schedule_id,
            } => {
                self.scheduled.retain(|candidate| {
                    !(candidate.schedule_id == Some(schedule_id)
                        && render_command_node_id(&candidate.kind) == Some(node_id))
                });
            }
        }
    }

    fn node_slot_mut(&mut self, node_id: u64) -> Option<&mut Option<BlockVoiceNode>> {
        self.nodes
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|node| node.id == node_id))
    }

    fn node_mut(&mut self, node_id: u64) -> Option<&mut BlockVoiceNode> {
        self.node_slot_mut(node_id).and_then(Option::as_mut)
    }

    fn render_segment(&mut self, start_frame: usize, frames: usize, start_sample: u64) {
        let channels = usize::from(self.channels.get());
        for local_frame in 0..frames {
            let frame = start_frame + local_frame;
            let frame_start = frame * channels;
            let frame_end = frame_start + channels;
            let absolute_sample = start_sample + local_frame as u64;
            let output = &mut self.block[frame_start..frame_end];
            for slot in &mut self.nodes {
                let Some(node) = slot.as_mut() else {
                    continue;
                };
                if !node.render_frame(output, absolute_sample) {
                    let node = slot.take().expect("node checked");
                    node.state.finished.store(true, Ordering::Release);
                }
            }
            // Device safety is owned once, after the complete voice/room mix. Normal material below
            // unity is unaffected; only overloaded frames receive stereo-linked gain reduction.
            self.output_limiter.process_frame(output);
        }
    }
}

impl Iterator for NativeBlockRenderSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.block_cursor >= self.block.len() {
            self.render_next_block();
        }
        let sample = self.block[self.block_cursor];
        self.block_cursor += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for NativeBlockRenderSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
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

fn finite_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 16.0)
    } else {
        1.0
    }
}

fn finite_speed(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.05, 4.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[path = "block_render/tests.rs"]
mod tests;
