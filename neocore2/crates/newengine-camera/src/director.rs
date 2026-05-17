#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{CameraBlendSpec, CameraDirectorMetadata, CameraResolvedFrame};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Render lifecycle for a camera director.
///
/// This mirrors the high-level reference shape: a director can blend in, fully render, blend out,
/// or stay inactive. The engine exposes it as data so runtime managers can arbitrate without
/// renderer-specific branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraRenderState {
    InterpolatingIn,
    FullyRendering,
    InterpolatingOut,
    NotRendering,
}

impl Default for CameraRenderState {
    #[inline]
    fn default() -> Self {
        Self::NotRendering
    }
}

impl CameraRenderState {
    #[inline]
    pub const fn is_rendering(self) -> bool {
        !matches!(self, Self::NotRendering)
    }

    #[inline]
    pub const fn is_interpolating(self) -> bool {
        matches!(self, Self::InterpolatingIn | Self::InterpolatingOut)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraDirectorId(pub u64);

#[derive(Clone, Copy, Debug)]
pub struct CameraDirectorOutput {
    pub id: CameraDirectorId,
    pub frame: CameraResolvedFrame,
    pub render_state: CameraRenderState,
    pub priority: i32,
    pub blend_level: f32,
    pub lock_input: bool,
}

impl CameraDirectorOutput {
    #[inline]
    pub fn is_rendering(&self) -> bool {
        self.render_state.is_rendering()
    }
}

/// Mutable render-state capsule shared by concrete director runners.
#[derive(Clone, Debug)]
pub struct CameraDirectorState {
    pub id: CameraDirectorId,
    pub metadata: CameraDirectorMetadata,
    pub render_state: CameraRenderState,
    pub elapsed_sec: f32,
    pub bypass_this_update: bool,
    pub blend: CameraBlendSpec,
}

impl CameraDirectorState {
    #[inline]
    pub fn new(id: CameraDirectorId, metadata: CameraDirectorMetadata) -> Self {
        let blend = metadata.blend.sanitized();
        Self {
            id,
            metadata,
            render_state: CameraRenderState::NotRendering,
            elapsed_sec: 0.0,
            bypass_this_update: false,
            blend,
        }
    }

    #[inline]
    pub fn render(&mut self, blend_in_sec: Option<f32>) {
        let duration = blend_in_sec.unwrap_or(self.blend.blend_in_sec);
        self.elapsed_sec = 0.0;
        self.render_state = if duration > 0.0 {
            CameraRenderState::InterpolatingIn
        } else {
            CameraRenderState::FullyRendering
        };
    }

    #[inline]
    pub fn stop_rendering(&mut self, blend_out_sec: Option<f32>) {
        let duration = blend_out_sec.unwrap_or(self.blend.blend_out_sec);
        self.elapsed_sec = 0.0;
        self.render_state = if duration > 0.0 && self.render_state.is_rendering() {
            CameraRenderState::InterpolatingOut
        } else {
            CameraRenderState::NotRendering
        };
    }

    #[inline]
    pub fn bypass_rendering_this_update(&mut self) {
        self.bypass_this_update = true;
    }

    #[inline]
    pub fn advance(&mut self, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }

        self.elapsed_sec += dt;
        match self.render_state {
            CameraRenderState::InterpolatingIn => {
                if self.elapsed_sec >= self.blend.blend_in_sec.max(0.0) {
                    self.render_state = CameraRenderState::FullyRendering;
                    self.elapsed_sec = 0.0;
                }
            }
            CameraRenderState::InterpolatingOut => {
                if self.elapsed_sec >= self.blend.blend_out_sec.max(0.0) {
                    self.render_state = CameraRenderState::NotRendering;
                    self.elapsed_sec = 0.0;
                }
            }
            CameraRenderState::FullyRendering | CameraRenderState::NotRendering => {}
        }
    }

    #[inline]
    pub fn blend_level(&self) -> f32 {
        match self.render_state {
            CameraRenderState::InterpolatingIn => {
                ratio(self.elapsed_sec, self.blend.blend_in_sec)
            }
            CameraRenderState::InterpolatingOut => {
                1.0 - ratio(self.elapsed_sec, self.blend.blend_out_sec)
            }
            CameraRenderState::FullyRendering => 1.0,
            CameraRenderState::NotRendering => 0.0,
        }
    }

    #[inline]
    pub fn output(&mut self, frame: CameraResolvedFrame) -> Option<CameraDirectorOutput> {
        let bypass = self.bypass_this_update;
        self.bypass_this_update = false;
        if bypass || !self.render_state.is_rendering() {
            return None;
        }
        Some(CameraDirectorOutput {
            id: self.id,
            frame,
            render_state: self.render_state,
            priority: self.metadata.priority,
            blend_level: self.blend_level(),
            lock_input: self.blend.lock_input,
        })
    }
}

#[inline]
fn ratio(elapsed: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        1.0
    } else {
        (elapsed / duration).clamp(0.0, 1.0)
    }
}

/// Common trait for Rust-native camera director runners.
pub trait CameraDirectorRunner {
    fn state(&self) -> &CameraDirectorState;
    fn state_mut(&mut self) -> &mut CameraDirectorState;
    fn update_frame(&mut self, dt: f32) -> Option<CameraResolvedFrame>;

    #[inline]
    fn update(&mut self, dt: f32) -> Option<CameraDirectorOutput> {
        self.state_mut().advance(dt);
        let frame = self.update_frame(dt)?;
        self.state_mut().output(frame)
    }
}
