#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelFootSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelFootPoseSample {
    pub position_world: Vec3,
    pub velocity_world: Vec3,
    pub valid: bool,
}
impl Default for ModelFootPoseSample {
    fn default() -> Self {
        Self {
            position_world: Vec3::ZERO,
            velocity_world: Vec3::ZERO,
            valid: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelFootPoseState {
    pub revision: u64,
    pub left: ModelFootPoseSample,
    pub right: ModelFootPoseSample,
}
impl ModelFootPoseState {
    pub fn from_world_positions(
        revision: u64,
        left: Vec3,
        right: Vec3,
        previous: Option<Self>,
        dt: f32,
    ) -> Self {
        fn sample(current: Vec3, old: Option<ModelFootPoseSample>, dt: f32) -> ModelFootPoseSample {
            let valid = current.is_finite();
            let velocity_world = if valid && dt.is_finite() && dt > 1.0e-5 {
                old.filter(|s| s.valid)
                    .map(|s| (current - s.position_world) / dt)
                    .filter(|v| v.is_finite())
                    .unwrap_or(Vec3::ZERO)
            } else {
                Vec3::ZERO
            };
            ModelFootPoseSample {
                position_world: if valid { current } else { Vec3::ZERO },
                velocity_world,
                valid,
            }
        }
        Self {
            revision: revision.max(1),
            left: sample(left, previous.map(|p| p.left), dt),
            right: sample(right, previous.map(|p| p.right), dt),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelGroundPlane {
    pub point_world: Vec3,
    pub normal_world: Vec3,
    pub valid: bool,
}
impl Default for ModelGroundPlane {
    fn default() -> Self {
        Self {
            point_world: Vec3::ZERO,
            normal_world: Vec3::Y,
            valid: false,
        }
    }
}
impl ModelGroundPlane {
    pub fn new(point_world: Vec3, normal_world: Vec3) -> Self {
        let normal_world = if normal_world.is_finite() && normal_world.length_squared() > 1.0e-8 {
            normal_world.normalize()
        } else {
            Vec3::Y
        };
        Self {
            point_world,
            normal_world,
            valid: point_world.is_finite(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelFootGroundSample {
    pub plane: ModelGroundPlane,
    pub surface_key: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelFootGroundState {
    pub revision: u64,
    pub left: ModelFootGroundSample,
    pub right: ModelFootGroundSample,
}

impl ModelFootGroundState {
    #[inline]
    pub fn foot(self, side: ModelFootSide) -> ModelFootGroundSample {
        match side {
            ModelFootSide::Left => self.left,
            ModelFootSide::Right => self.right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelFootContactTuning {
    pub enter_distance: f32,
    pub exit_distance: f32,
    pub max_penetration: f32,
    pub min_retrigger_seconds: f32,
}
impl Default for ModelFootContactTuning {
    fn default() -> Self {
        Self {
            enter_distance: 0.050,
            exit_distance: 0.085,
            max_penetration: 0.18,
            min_retrigger_seconds: 0.045,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelFootContactSample {
    pub touching: bool,
    pub began: bool,
    pub ended: bool,
    pub point_world: Vec3,
    pub signed_distance: f32,
    pub normal_speed: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelFootContactFrame {
    pub left: ModelFootContactSample,
    pub right: ModelFootContactSample,
}

#[derive(Clone, Copy, Debug, Default)]
struct FootLatch {
    touching: bool,
    retrigger: f32,
}

#[derive(Clone, Debug, Default)]
pub struct ModelFootContactTracker {
    left: FootLatch,
    right: FootLatch,
}
impl ModelFootContactTracker {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(
        &mut self,
        pose: ModelFootPoseState,
        ground: ModelGroundPlane,
        dt: f32,
        tuning: ModelFootContactTuning,
    ) -> ModelFootContactFrame {
        ModelFootContactFrame {
            left: update_one(&mut self.left, pose.left, ground, dt, tuning),
            right: update_one(&mut self.right, pose.right, ground, dt, tuning),
        }
    }

    pub fn update_per_foot(
        &mut self,
        pose: ModelFootPoseState,
        ground: ModelFootGroundState,
        dt: f32,
        tuning: ModelFootContactTuning,
    ) -> ModelFootContactFrame {
        ModelFootContactFrame {
            left: update_one(&mut self.left, pose.left, ground.left.plane, dt, tuning),
            right: update_one(&mut self.right, pose.right, ground.right.plane, dt, tuning),
        }
    }
}

fn update_one(
    latch: &mut FootLatch,
    foot: ModelFootPoseSample,
    ground: ModelGroundPlane,
    dt: f32,
    tuning: ModelFootContactTuning,
) -> ModelFootContactSample {
    let enter = tuning.enter_distance.max(0.005);
    let exit = tuning.exit_distance.max(enter + 0.005);
    let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
    latch.retrigger = (latch.retrigger - dt).max(0.0);
    if !ground.valid || !foot.valid {
        let ended = latch.touching;
        latch.touching = false;
        return ModelFootContactSample {
            ended,
            ..Default::default()
        };
    }
    let n = ground.normal_world;
    let distance = (foot.position_world - ground.point_world).dot(n);
    let point = foot.position_world - n * distance;
    let before = latch.touching;
    if before {
        if distance > exit || distance < -tuning.max_penetration.abs().max(0.02) {
            latch.touching = false;
            latch.retrigger = tuning.min_retrigger_seconds.max(0.0);
        }
    } else if latch.retrigger <= 0.0
        && distance <= enter
        && distance >= -tuning.max_penetration.abs().max(0.02)
    {
        latch.touching = true;
    }
    ModelFootContactSample {
        touching: latch.touching,
        began: !before && latch.touching,
        ended: before && !latch.touching,
        point_world: point,
        signed_distance: distance,
        normal_speed: foot.velocity_world.dot(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(left_y: f32, right_y: f32) -> ModelFootPoseState {
        ModelFootPoseState::from_world_positions(
            1,
            Vec3::new(-0.1, left_y, 0.0),
            Vec3::new(0.1, right_y, 0.0),
            None,
            1.0 / 60.0,
        )
    }

    #[test]
    fn contact_edges_follow_animated_foot_height() {
        let mut tracker = ModelFootContactTracker::default();
        let ground = ModelGroundPlane::new(Vec3::ZERO, Vec3::Y);
        let t = ModelFootContactTuning::default();
        assert!(
            !tracker
                .update(pose(0.14, 0.14), ground, 1.0 / 60.0, t)
                .left
                .touching
        );
        let plant = tracker.update(pose(0.03, 0.14), ground, 1.0 / 60.0, t);
        assert!(plant.left.touching && plant.left.began);
        assert!(
            !tracker
                .update(pose(0.06, 0.14), ground, 1.0 / 60.0, t)
                .left
                .began
        );
        assert!(
            tracker
                .update(pose(0.11, 0.14), ground, 1.0 / 60.0, t)
                .left
                .ended
        );
    }

    #[test]
    fn audio_origin_is_projected_to_real_support_plane() {
        let mut tracker = ModelFootContactTracker::default();
        let ground = ModelGroundPlane::new(Vec3::new(0.0, 2.0, 0.0), Vec3::Y);
        let frame = tracker.update(
            ModelFootPoseState::from_world_positions(
                1,
                Vec3::new(-0.2, 2.03, 0.4),
                Vec3::new(0.2, 2.2, 0.4),
                None,
                1.0 / 60.0,
            ),
            ground,
            1.0 / 60.0,
            ModelFootContactTuning::default(),
        );
        assert!(frame.left.began);
        assert!((frame.left.point_world.y - 2.0).abs() < 1.0e-6);
    }
}
