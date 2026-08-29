use super::*;

impl AssetInspectorRuntimeModule {
    pub(super) fn begin_activity(&mut self, label: impl Into<String>, frame_index: u64) {
        self.activity = Some(InspectorActivity {
            label: label.into(),
            started_frame: frame_index,
            completed_frame: None,
            waiting_for_preview: false,
            last_published_frame: frame_index,
        });
        self.dirty = true;
    }
    pub(super) fn complete_activity(&mut self, frame_index: u64) {
        if let Some(activity) = self.activity.as_mut() {
            activity.waiting_for_preview = false;
            activity.completed_frame.get_or_insert(frame_index);
            self.dirty = true;
        }
    }
    pub(super) fn finish_activity_after_preview_request(&mut self, frame_index: u64) {
        let waiting_for_preview = self
            .preview_snapshot
            .as_ref()
            .is_some_and(|preview| preview.kind == AssetPreviewKind::Scene3d && !preview.ready);
        if let Some(activity) = self.activity.as_mut() {
            activity.waiting_for_preview = waiting_for_preview;
            if waiting_for_preview {
                activity.label = "RENDERING".to_owned();
                activity.completed_frame = None;
            } else {
                activity.completed_frame.get_or_insert(frame_index);
            }
            self.dirty = true;
        }
    }
    pub(super) fn tick_activity(&mut self, frame_index: u64) {
        let expired = self.activity.as_ref().is_some_and(|activity| {
            activity.completed_frame.is_some_and(|completed| {
                frame_index.saturating_sub(completed)
                    > ACTIVITY_COMPLETE_ANIMATION_FRAMES + ACTIVITY_COMPLETE_HOLD_FRAMES
            })
        });
        if expired {
            self.activity = None;
            self.dirty = true;
            return;
        }
        if let Some(activity) = self.activity.as_mut() {
            let publish_due = frame_index.saturating_sub(activity.last_published_frame)
                >= ACTIVITY_PUBLISH_INTERVAL_FRAMES;
            if publish_due {
                activity.last_published_frame = frame_index;
                self.dirty = true;
            }
        }
    }
    pub(super) fn activity_view(&self, frame_index: u64) -> (f32, &str) {
        let Some(activity) = self.activity.as_ref() else {
            return (1.0, "READY");
        };
        let progress_01 = inspector_activity_progress_01(activity, frame_index);
        let label = match activity.completed_frame {
            Some(completed)
                if frame_index.saturating_sub(completed) < ACTIVITY_COMPLETE_ANIMATION_FRAMES =>
            {
                "FINALIZING"
            }
            Some(_) => "READY",
            None => activity.label.as_str(),
        };
        (progress_01, label)
    }
}

pub(super) fn inspector_activity_running_progress_01(elapsed_frames: u64) -> f32 {
    let elapsed = elapsed_frames as f32;
    (0.08 + 0.82 * (1.0 - (-elapsed / 42.0).exp())).clamp(0.08, 0.90)
}
pub(super) fn inspector_activity_progress_01(
    activity: &InspectorActivity,
    frame_index: u64,
) -> f32 {
    let running =
        inspector_activity_running_progress_01(frame_index.saturating_sub(activity.started_frame));
    let Some(completed_frame) = activity.completed_frame else {
        return running;
    };
    let completed_elapsed = frame_index.saturating_sub(completed_frame);
    let t = (completed_elapsed as f32 / ACTIVITY_COMPLETE_ANIMATION_FRAMES as f32).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (running + (1.0 - running) * smooth).clamp(0.0, 1.0)
}
