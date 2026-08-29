use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayJumpTarget {
    pub clip_index: u32,
    pub clip_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayPlaybackClockSnapshot {
    pub frame_index: u64,
    pub clip_index: u32,
    pub clip_time_ms: u64,
    pub pending_jump: Option<ReplayJumpTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlaybackClock {
    frame_index: u64,
    clip_index: u32,
    clip_time_ms: u64,
    pending_jump: Option<ReplayJumpTarget>,
}

impl Default for ReplayPlaybackClock {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayPlaybackClock {
    #[inline]
    pub const fn new() -> Self {
        Self {
            frame_index: 0,
            clip_index: 0,
            clip_time_ms: 0,
            pending_jump: None,
        }
    }

    #[inline]
    pub const fn snapshot(&self) -> ReplayPlaybackClockSnapshot {
        ReplayPlaybackClockSnapshot {
            frame_index: self.frame_index,
            clip_index: self.clip_index,
            clip_time_ms: self.clip_time_ms,
            pending_jump: self.pending_jump,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[inline]
    pub fn jump_to_clip(&mut self, clip_index: u32, clip_time_ms: u64) {
        self.pending_jump = Some(ReplayJumpTarget {
            clip_index,
            clip_time_ms,
        });
    }

    #[inline]
    pub fn clear_pending_jump(&mut self) {
        self.pending_jump = None;
    }

    pub fn advance_fixed(&mut self, frame_duration_ms: u64) -> ReplayPlaybackClockSnapshot {
        if let Some(jump) = self.pending_jump.take() {
            self.clip_index = jump.clip_index;
            self.clip_time_ms = jump.clip_time_ms;
        } else {
            self.clip_time_ms = self.clip_time_ms.saturating_add(frame_duration_ms);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        self.snapshot()
    }
}
