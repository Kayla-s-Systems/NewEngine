#![forbid(unsafe_op_in_unsafe_fn)]

use super::SharedVoiceState;
use rodio::source::{SeekError, Source};
use rodio::{ChannelCount, Sample, SampleRate};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

struct GainRamp {
    from: f32,
    target: f32,
    start_sample: u64,
    end_sample: u64,
}

pub(super) struct BlockVoiceNodeInit {
    pub(super) id: u64,
    pub(super) source: BlockSourceAdapter,
    pub(super) gain: f32,
    pub(super) speed: f32,
    pub(super) paused: bool,
    pub(super) source_position: Duration,
    pub(super) state: Arc<SharedVoiceState>,
    pub(super) sample_rate: SampleRate,
    pub(super) channels: ChannelCount,
}

pub(super) struct BlockVoiceNode {
    pub(super) id: u64,
    source: BlockSourceAdapter,
    gain: f32,
    pub(super) speed: f32,
    pub(super) paused: bool,
    output_position_seconds: f64,
    pub(super) state: Arc<SharedVoiceState>,
    sample_rate: SampleRate,
    scratch: Vec<Sample>,
    gain_ramp: Option<GainRamp>,
}

impl BlockVoiceNode {
    pub(super) fn new(init: BlockVoiceNodeInit) -> Self {
        let BlockVoiceNodeInit {
            id,
            source,
            gain,
            speed,
            paused,
            source_position,
            state,
            sample_rate,
            channels,
        } = init;
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

    pub(super) fn set_gain(&mut self, gain: f32) {
        self.gain = finite_gain(gain);
        self.gain_ramp = None;
    }

    pub(super) fn ramp_gain(&mut self, target: f32, start_sample: u64, duration_samples: u64) {
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

    pub(super) fn seek(&mut self, position: Duration) {
        if self.source.try_seek(position).is_ok() {
            self.output_position_seconds = position.as_secs_f64();
            self.publish_position();
            self.state.finished.store(false, Ordering::Release);
        }
    }

    pub(super) fn render_frame(&mut self, output: &mut [Sample], absolute_sample: u64) -> bool {
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
        true
    }

    pub(super) fn publish_position(&self) {
        let nanos = (self.output_position_seconds.max(0.0) * 1_000_000_000.0)
            .round()
            .clamp(0.0, u64::MAX as f64) as u64;
        self.state
            .source_position_ns
            .store(nanos, Ordering::Release);
    }
}

pub(super) struct BlockSourceAdapter {
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
    pub(super) fn new(source: Box<dyn Source + Send>, channels: ChannelCount) -> Self {
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
        for ((output, current), next) in out
            .iter_mut()
            .zip(self.current.iter())
            .zip(self.next.iter())
            .take(self.channels)
        {
            *output = *current + (*next - *current) * t;
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

pub(super) fn finite_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 16.0)
    } else {
        1.0
    }
}

pub(super) fn finite_speed(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.05, 4.0)
    } else {
        1.0
    }
}
