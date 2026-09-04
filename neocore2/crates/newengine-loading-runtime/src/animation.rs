use std::f32::consts::TAU;
use std::time::Instant;

use newengine_loading_api::LoadingScreenSnapshot;

#[derive(Debug, Clone)]
pub struct LoadingProjectionFrame {
    pub snapshot: LoadingScreenSnapshot,
    pub visual_progress_01: f32,
    pub spinner_angle_rad: f32,
    pub pulse_01: f32,
    pub age_secs: f32,
    pub detail_age_secs: f32,
}

/// Backward-compatible type name for platform-native loading projection frames.
///
/// The data remains owned by `engine.loading`; platform code only presents the
/// already-projected DTO during native startup handoff.
pub type LoadingCompositorFrame = LoadingProjectionFrame;

#[derive(Debug, Clone)]
pub struct LoadingAnimator {
    created_at: Instant,
    last_present_at: Instant,
    detail_started_at: Instant,
    last_detail_key: String,
    visible_detail_key: String,
    visible_title: String,
    visible_status: String,
    visible_detail: String,
    visual_progress_01: f32,
}

const MIN_STATUS_HOLD_SECS: f32 = 0.55;

impl Default for LoadingAnimator {
    #[inline]
    fn default() -> Self {
        let now = Instant::now();
        Self {
            created_at: now,
            last_present_at: now,
            detail_started_at: now,
            last_detail_key: String::new(),
            visible_detail_key: String::new(),
            visible_title: String::new(),
            visible_status: String::new(),
            visible_detail: String::new(),
            visual_progress_01: 0.0,
        }
    }
}

impl LoadingAnimator {
    pub fn present(&mut self, snapshot: &LoadingScreenSnapshot) -> LoadingProjectionFrame {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_present_at)
            .as_secs_f32()
            .clamp(1.0 / 240.0, 1.0 / 20.0);
        self.last_present_at = now;

        let mut next = snapshot.clone().normalize();
        let detail_key = format!("{}\n{}\n{}", next.title, next.status, next.detail);
        if self.last_detail_key != detail_key {
            self.last_detail_key = detail_key.clone();
        }

        let visible_age = now
            .saturating_duration_since(self.detail_started_at)
            .as_secs_f32();
        let can_switch_text = self.visible_detail_key.is_empty()
            || visible_age >= MIN_STATUS_HOLD_SECS
            || !next.active;
        if self.visible_detail_key != detail_key && can_switch_text {
            self.visible_detail_key = detail_key;
            self.visible_title = next.title.clone();
            self.visible_status = next.status.clone();
            self.visible_detail = next.detail.clone();
            self.detail_started_at = now;
        } else if !self.visible_detail_key.is_empty() && self.visible_detail_key != detail_key {
            next.title = self.visible_title.clone();
            next.status = self.visible_status.clone();
            next.detail = self.visible_detail.clone();
        }

        let target = next.progress_01.clamp(0.0, 1.0);
        let target = if next.active {
            target.max(self.visual_progress_01)
        } else {
            target
        };
        let speed = if target < self.visual_progress_01 {
            16.0
        } else {
            5.6
        };
        let alpha = 1.0 - (-speed * dt).exp();
        self.visual_progress_01 += (target - self.visual_progress_01) * alpha;
        self.visual_progress_01 = self.visual_progress_01.clamp(0.0, 1.0);

        let age = now.saturating_duration_since(self.created_at).as_secs_f32();
        // Spinner animation must be driven by loading-projection monotonic time,
        // not by task/status events. Event-driven phases arrive irregularly during
        // heavy startup work, which made the spinner advance in visible bursts.
        // The bus-provided phase is kept only as a tiny deterministic offset so
        // status publishers can still decorrelate multiple loading surfaces.
        let spinner_phase_offset = ((next.spinner_phase as f32 % 144.0) / 144.0) * TAU;
        let spinner_angle_rad = (spinner_phase_offset + age * 1.35 * TAU) % TAU;
        let pulse_01 = ((age * 1.25 * TAU).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

        LoadingProjectionFrame {
            snapshot: next,
            visual_progress_01: self.visual_progress_01,
            spinner_angle_rad,
            pulse_01,
            age_secs: age,
            detail_age_secs: now
                .saturating_duration_since(self.detail_started_at)
                .as_secs_f32(),
        }
    }
}
