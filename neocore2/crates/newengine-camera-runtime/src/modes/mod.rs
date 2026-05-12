#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{Quat, Vec3};
use newengine_sim::FollowTargetCameraController;

#[derive(Clone, Copy, Debug)]
pub struct GameplayFirstPersonRunner {
    pub eye_height: f32,
}

impl Default for GameplayFirstPersonRunner {
    #[inline]
    fn default() -> Self {
        Self { eye_height: 1.6 }
    }
}

impl GameplayFirstPersonRunner {
    #[inline]
    pub fn controller(self, player: EntityId) -> FollowTargetCameraController {
        FollowTargetCameraController {
            target: player,
            offset_ls: Vec3::new(0.0, self.eye_height.max(0.01), 0.0),
            rot_offset: Quat::IDENTITY,
            follow_rotation: true,
            smooth_time: 0.0,
            max_speed: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GameplayThirdPersonFollowRunner {
    pub shoulder_offset: Vec3,
    pub smooth_time: f32,
    pub max_speed: f32,
}

impl Default for GameplayThirdPersonFollowRunner {
    #[inline]
    fn default() -> Self {
        Self {
            shoulder_offset: Vec3::new(0.35, 1.65, 4.5),
            smooth_time: 0.08,
            max_speed: 0.0,
        }
    }
}

impl GameplayThirdPersonFollowRunner {
    #[inline]
    pub fn controller(self, player: EntityId) -> FollowTargetCameraController {
        FollowTargetCameraController {
            target: player,
            offset_ls: self.shoulder_offset,
            rot_offset: Quat::IDENTITY,
            follow_rotation: false,
            smooth_time: self.smooth_time.max(0.0),
            max_speed: self.max_speed.max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GameplayThirdPersonAimRunner {
    pub shoulder_offset: Vec3,
    pub smooth_time: f32,
    pub max_speed: f32,
}

impl Default for GameplayThirdPersonAimRunner {
    #[inline]
    fn default() -> Self {
        Self {
            shoulder_offset: Vec3::new(0.55, 1.55, 2.2),
            smooth_time: 0.035,
            max_speed: 0.0,
        }
    }
}

impl GameplayThirdPersonAimRunner {
    #[inline]
    pub fn controller(self, player: EntityId) -> FollowTargetCameraController {
        FollowTargetCameraController {
            target: player,
            offset_ls: self.shoulder_offset,
            rot_offset: Quat::IDENTITY,
            follow_rotation: true,
            smooth_time: self.smooth_time.max(0.0),
            max_speed: self.max_speed.max(0.0),
        }
    }
}
