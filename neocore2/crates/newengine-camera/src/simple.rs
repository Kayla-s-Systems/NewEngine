#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2, Vec3};

use crate::{
    CameraChannel, CameraChannelState, CameraClipPolicy, CameraControlInput, CameraFrame,
    CameraLens, CameraWorldFrame, CameraWorldRig, CameraRig, CameraViewport,
    CameraWorldPoint, FreeFlyController, OrbitController, Projection,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Small, batteries-included camera facade.
///
/// This is the intentionally simple entry point: create it, feed optional input, ask for a frame.
/// Advanced runtime/provider code can still use directors, stacks and gateway snapshots directly.
#[derive(Clone, Debug)]
pub struct Camera {
    pub rig: CameraRig,
    pub projection: Projection,
    pub viewport: CameraViewport,
    pub channel: CameraChannelState,
    pub jitter_px: Vec2,
}

impl Default for Camera {
    #[inline]
    fn default() -> Self {
        Self::perspective(CameraViewport::default(), CameraLens::default())
    }
}

impl Camera {
    #[inline]
    pub fn perspective(viewport: CameraViewport, lens: CameraLens) -> Self {
        let viewport = viewport.sanitized();
        Self {
            rig: CameraRig::default(),
            projection: lens.projection(viewport.aspect()),
            viewport,
            channel: CameraChannelState::dominant(CameraChannel::Gameplay),
            jitter_px: Vec2::ZERO,
        }
    }

    #[inline]
    pub fn from_size(width: u32, height: u32) -> Self {
        Self::perspective(CameraViewport::from_size(width, height), CameraLens::default())
    }

    #[inline]
    pub fn with_channel(mut self, channel: CameraChannel) -> Self {
        self.channel = CameraChannelState::dominant(channel);
        self
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = CameraViewport::from_size(width, height);
        self.projection.set_viewport(width, height);
    }

    #[inline]
    pub fn set_pose(&mut self, position: Vec3, rotation: Quat) {
        self.rig = CameraRig::new(position, rotation.normalize_or_identity());
    }

    #[inline]
    pub fn look_at(&mut self, position: Vec3, target: Vec3) {
        self.rig.set_look_at(position, target, Vec3::Y);
    }

    #[inline]
    pub fn translate_local(&mut self, delta: Vec3) {
        self.rig.translate_local(delta);
    }

    #[inline]
    pub fn set_lens(&mut self, lens: CameraLens) {
        self.projection = lens.projection(self.viewport.aspect());
    }

    #[inline]
    pub fn update_clip_for_focus(&mut self, policy: CameraClipPolicy, distance: f32, radius: f32, max_far: f32) {
        let (near, far) = policy.near_far(distance, radius, max_far);
        self.projection.set_near_far(near, far);
    }

    #[inline]
    pub fn frame(&self) -> CameraFrame {
        CameraFrame::build(self.channel, self.rig, self.projection, self.viewport, self.jitter_px)
    }
}

/// Simple controller mode for `CameraController`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraControllerMode {
    Orbit,
    Fly,
}

impl Default for CameraControllerMode {
    #[inline]
    fn default() -> Self {
        Self::Orbit
    }
}

/// A minimal orbit/fly controller wrapper for tools and demos.
#[derive(Clone, Copy, Debug)]
pub struct CameraController {
    pub mode: CameraControllerMode,
    pub orbit: OrbitController,
    pub fly: FreeFlyController,
}

impl Default for CameraController {
    #[inline]
    fn default() -> Self {
        Self { mode: CameraControllerMode::Orbit, orbit: OrbitController::default(), fly: FreeFlyController::default() }
    }
}

impl CameraController {
    #[inline]
    pub fn set_mode(&mut self, mode: CameraControllerMode, rig: &CameraRig) {
        if self.mode == mode {
            return;
        }
        match mode {
            CameraControllerMode::Orbit => self.orbit.sync_from_rig(rig),
            CameraControllerMode::Fly => self.fly.sync_from_rig(rig),
        }
        self.mode = mode;
    }

    #[inline]
    pub fn apply(&mut self, camera: &mut Camera, input: CameraControlInput, dt: f32) {
        match self.mode {
            CameraControllerMode::Orbit => self.orbit.apply(&mut camera.rig, input, dt),
            CameraControllerMode::Fly => self.fly.apply(&mut camera.rig, input, dt),
        }
    }
}

/// World-space version of the simple camera facade.
///
/// The public position is `f64`; `frame()` emits camera-origin-relative `f32` data.
#[derive(Clone, Debug)]
pub struct WorldCamera {
    pub rig: CameraWorldRig,
    pub projection: Projection,
    pub viewport: CameraViewport,
    pub channel: CameraChannelState,
    pub jitter_px: Vec2,
    pub clip_policy: CameraClipPolicy,
}

impl WorldCamera {
    #[inline]
    pub fn new(position: CameraWorldPoint, viewport: CameraViewport, lens: CameraLens) -> Self {
        let viewport = viewport.sanitized();
        Self {
            rig: CameraWorldRig::new(position, Quat::IDENTITY),
            projection: lens.projection(viewport.aspect()),
            viewport,
            channel: CameraChannelState::dominant(CameraChannel::Gameplay),
            jitter_px: Vec2::ZERO,
            clip_policy: CameraClipPolicy::world_space(),
        }
    }

    #[inline]
    pub fn from_size(width: u32, height: u32) -> Self {
        Self::new(CameraWorldPoint::ZERO, CameraViewport::from_size(width, height), CameraLens::default())
    }

    #[inline]
    pub fn set_position(&mut self, position: CameraWorldPoint) {
        self.rig.position = position;
        self.rig.rebase_if_needed();
    }

    #[inline]
    pub fn look_at(&mut self, position: CameraWorldPoint, target: CameraWorldPoint) {
        self.rig.position = position;
        self.rig.rebase_if_needed();
        let local_pos = self.rig.origin.relative_point(position);
        let local_target = self.rig.origin.relative_point(target);
        self.rig.rotation = crate::CameraRig::from_look_at(local_pos, local_target, Vec3::Y).rotation;
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = CameraViewport::from_size(width, height);
        self.projection.set_viewport(width, height);
    }

    #[inline]
    pub fn with_channel(mut self, channel: CameraChannel) -> Self {
        self.channel = CameraChannelState::dominant(channel);
        self
    }

    #[inline]
    pub fn set_lens(&mut self, lens: CameraLens) {
        self.projection = lens.projection(self.viewport.aspect());
    }

    #[inline]
    pub fn set_origin_cell_size(&mut self, cell_size: f64) {
        self.rig = self.rig.with_cell_size(cell_size);
    }

    #[inline]
    pub fn translate_world(&mut self, delta: Vec3) {
        if delta.is_finite() {
            self.rig.position = self.rig.position.translated(delta);
            self.rig.rebase_if_needed();
        }
    }

    #[inline]
    pub fn update_clip_for_focus(&mut self, distance: f32, radius: f32, max_far: f32) {
        let (near, far) = self.clip_policy.near_far(distance, radius, max_far);
        self.projection.set_near_far(near, far);
    }

    #[inline]
    pub fn frame(&self) -> CameraWorldFrame {
        CameraWorldFrame::build(self.channel, self.rig, self.projection, self.viewport, self.jitter_px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_camera_builds_finite_frame() {
        let mut camera = Camera::from_size(1280, 720);
        camera.look_at(Vec3::new(0.0, 1.0, 3.0), Vec3::ZERO);
        let frame = camera.frame();
        assert!(frame.diagnostics.finite);
        assert_eq!(frame.viewport.width, 1280);
    }

    #[test]
    fn world_camera_emits_local_frame() {
        let camera = WorldCamera::new(
            CameraWorldPoint::new(10_000_000.0, 10.0, -10_000_000.0),
            CameraViewport::from_size(1920, 1080),
            CameraLens::default(),
        );
        let frame = camera.frame();
        assert!(frame.frame.rig.position.x.abs() < 1024.0);
        assert!(frame.frame.rig.position.z.abs() < 1024.0);
    }
}
