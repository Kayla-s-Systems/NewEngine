#![forbid(unsafe_op_in_unsafe_fn)]

use crate::manager::CameraDirectorKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraRuntimeEventKind {
    DirectorRequested,
    DirectorActivated,
    DirectorDeactivated,
    DirectorBypassed,
    TransitionStarted,
    TransitionCompleted,
    DominantDirectorChanged,
    EffectsChanged,
    ViewportChanged,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CameraRuntimeEvent {
    pub kind: CameraRuntimeEventKind,
    pub director: CameraDirectorKind,
    pub previous_director: Option<CameraDirectorKind>,
    pub blend_level: f32,
    pub elapsed_sec: f32,
    pub message: Option<String>,
}

impl CameraRuntimeEvent {
    #[inline]
    pub fn new(kind: CameraRuntimeEventKind, director: CameraDirectorKind) -> Self {
        Self {
            kind,
            director,
            previous_director: None,
            blend_level: 1.0,
            elapsed_sec: 0.0,
            message: None,
        }
    }

    #[inline]
    pub fn with_previous(mut self, previous: Option<CameraDirectorKind>) -> Self {
        self.previous_director = previous;
        self
    }

    #[inline]
    pub fn with_blend(mut self, blend_level: f32) -> Self {
        self.blend_level = if blend_level.is_finite() {
            blend_level.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    #[inline]
    pub fn with_elapsed(mut self, elapsed_sec: f32) -> Self {
        self.elapsed_sec = if elapsed_sec.is_finite() && elapsed_sec > 0.0 {
            elapsed_sec
        } else {
            0.0
        };
        self
    }

    #[inline]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}
