use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use rodio::source::{SeekError, Source, UniformSourceIterator};
use rodio::Sample;
use rodio::{ChannelCount, SampleRate};

#[path = "block_render/commands.rs"]
mod commands;

#[path = "block_render/limiter.rs"]
mod limiter;
#[path = "block_render/voice.rs"]
mod voice;

use commands::{render_command_node_id, RenderCommand, RenderCommandKind};
use limiter::OutputPeakLimiter;
use voice::{
    finite_gain, finite_speed, BlockSourceAdapter, BlockVoiceNode, BlockVoiceNodeInit,
};

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
                node: BlockVoiceNode::new(BlockVoiceNodeInit {
                    id: node_id,
                    source: adapter,
                    gain: finite_gain(gain),
                    speed: finite_speed(speed),
                    paused,
                    source_position,
                    state: Arc::clone(&state),
                    sample_rate: self.sample_rate,
                    channels: self.channels,
                }),
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

pub(crate) struct NativeBlockRenderSource {
    channels: ChannelCount,
    sample_rate: SampleRate,
    command_rx: Receiver<RenderCommand>,
    stats: Arc<SharedRenderStats>,
    /// Dense active-node storage keeps the per-frame mixer proportional to active voices,
    /// not the configured voice ceiling.
    nodes: Vec<BlockVoiceNode>,
    node_index_by_id: HashMap<u64, usize>,
    scheduled: BinaryHeap<Reverse<RenderCommand>>,
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
        Self {
            channels,
            sample_rate,
            command_rx,
            stats,
            nodes: Vec::with_capacity(MAX_BLOCK_NODES),
            node_index_by_id: HashMap::with_capacity(MAX_BLOCK_NODES),
            scheduled: BinaryHeap::with_capacity(MAX_RENDER_COMMANDS),
            block: vec![0.0; NATIVE_BLOCK_FRAMES * usize::from(channels.get())],
            block_cursor: NATIVE_BLOCK_FRAMES * usize::from(channels.get()),
            output_sample: 0,
            output_limiter: OutputPeakLimiter::new(sample_rate),
        }
    }

    fn drain_commands(&mut self) {
        while let Ok(mut command) = self.command_rx.try_recv() {
            command.at_sample = Some(command.resolved_sample(self.output_sample));
            if self.scheduled.len() >= MAX_RENDER_COMMANDS {
                self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                if let RenderCommandKind::Add { node } = command.kind {
                    node.state.finished.store(true, Ordering::Release);
                }
                continue;
            }
            // The render callback never re-sorts the complete schedule. A preallocated
            // min-heap keeps admission O(log N) and the next due command O(1) to inspect.
            self.scheduled.push(Reverse(command));
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
                .peek()
                .and_then(|entry| entry.0.at_sample)
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
        self.stats
            .active_nodes
            .store(self.nodes.len() as u64, Ordering::Relaxed);
        self.block_cursor = 0;
    }

    fn apply_due_commands(&mut self, sample: u64) {
        while self
            .scheduled
            .peek()
            .is_some_and(|entry| entry.0.at_sample.unwrap_or(sample) <= sample)
        {
            let Reverse(command) = self
                .scheduled
                .pop()
                .expect("scheduled command checked by peek()");
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
                if self.nodes.len() < MAX_BLOCK_NODES {
                    let index = self.nodes.len();
                    node.state.finished.store(false, Ordering::Release);
                    self.node_index_by_id.insert(node.id, index);
                    self.nodes.push(node);
                } else {
                    node.state.finished.store(true, Ordering::Release);
                    self.stats.dropped_commands.fetch_add(1, Ordering::Relaxed);
                }
            }
            RenderCommandKind::Remove { node_id } => {
                if let Some(node) = self.remove_node(node_id) {
                    node.state.finished.store(true, Ordering::Release);
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
                self.scheduled.retain(|entry| {
                    let candidate = &entry.0;
                    !(candidate.schedule_id == Some(schedule_id)
                        && render_command_node_id(&candidate.kind) == Some(node_id))
                });
            }
        }
    }

    fn remove_node(&mut self, node_id: u64) -> Option<BlockVoiceNode> {
        let index = self.node_index_by_id.remove(&node_id)?;
        let removed = self.nodes.swap_remove(index);
        if index < self.nodes.len() {
            let moved_id = self.nodes[index].id;
            self.node_index_by_id.insert(moved_id, index);
        }
        Some(removed)
    }

    fn node_mut(&mut self, node_id: u64) -> Option<&mut BlockVoiceNode> {
        let index = *self.node_index_by_id.get(&node_id)?;
        self.nodes.get_mut(index)
    }

    fn render_segment(&mut self, start_frame: usize, frames: usize, start_sample: u64) {
        let channels = usize::from(self.channels.get());
        for local_frame in 0..frames {
            let frame = start_frame + local_frame;
            let frame_start = frame * channels;
            let frame_end = frame_start + channels;
            let absolute_sample = start_sample + local_frame as u64;
            let output = &mut self.block[frame_start..frame_end];
            let mut node_index = 0usize;
            while node_index < self.nodes.len() {
                if self.nodes[node_index].render_frame(output, absolute_sample) {
                    node_index += 1;
                    continue;
                }
                let node_id = self.nodes[node_index].id;
                let removed_index = self
                    .node_index_by_id
                    .remove(&node_id)
                    .expect("dense block node must be indexed");
                debug_assert_eq!(removed_index, node_index);
                let node = self.nodes.swap_remove(node_index);
                if node_index < self.nodes.len() {
                    let moved_id = self.nodes[node_index].id;
                    self.node_index_by_id.insert(moved_id, node_index);
                }
                // Publish the final successfully rendered source position before retiring the node.
                node.publish_position();
                node.state.finished.store(true, Ordering::Release);
            }
            // Device safety is owned once, after the complete voice/room mix. Normal material below
            // unity is unaffected; only overloaded frames receive stereo-linked gain reduction.
            self.output_limiter.process_frame(output);
        }
        // render_next_block() pre-renders the segment synchronously, so intermediate per-frame
        // atomics are not externally useful. Publish once at the segment boundary instead.
        for node in &self.nodes {
            node.publish_position();
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

#[cfg(test)]
#[path = "block_render/tests.rs"]
mod tests;
