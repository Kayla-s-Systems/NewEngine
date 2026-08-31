use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    CharacterBody, CollisionShapeDesc, PhysicsBodyDesc, PlayerGroundState, PlayerMovementSpeeds,
    PlayerStanceKind, PlayerStanceState, StaticMeshCollider,
};
use newengine_math::{Vec2, Vec3};
use newengine_model_contact_api::{
    ModelFootContactFrame, ModelFootContactTracker, ModelFootContactTuning, ModelFootGroundSample,
    ModelFootGroundState, ModelFootPoseState, ModelGroundPlane,
};
use newengine_sim::MotorInput;
use newengine_transform::Transform;

/// Runtime locomotion mode is a physical/gameplay fact. It is intentionally not an audio cue,
/// filename, event id, or asset selector. Projects decide what to do with the published facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FootstepLocomotionMode {
    Walk,
    Run,
    Sprint,
    Stealth,
    Land,
}

impl FootstepLocomotionMode {
    #[inline]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Sprint => "sprint",
            Self::Stealth => "stealth",
            Self::Land => "land",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FootSide {
    #[default]
    Left,
    Right,
}

impl FootSide {
    #[inline]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    #[inline]
    pub(crate) const fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }

    #[inline]
    pub(crate) const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FootstepPhase {
    Contact,
    Scuff,
    Land,
}

impl FootstepPhase {
    #[inline]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Contact => "contact",
            Self::Scuff => "scuff",
            Self::Land => "land",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FootstepRuntimeState {
    pub(crate) next_foot: FootSide,
    pub(crate) sequence: u64,
    pub(crate) last_direction: Vec2,
    pub(crate) last_mode: Option<FootstepLocomotionMode>,
    pub(crate) was_moving: bool,
    pub(crate) scuff_cooldown: f32,
    pub(crate) model_contacts: ModelFootContactTracker,
}

impl Default for FootstepRuntimeState {
    fn default() -> Self {
        Self {
            next_foot: FootSide::Left,
            sequence: 0,
            last_direction: Vec2::ZERO,
            last_mode: None,
            was_moving: false,
            scuff_cooldown: 0.0,
            model_contacts: ModelFootContactTracker::default(),
        }
    }
}

impl FootstepRuntimeState {
    #[inline]
    pub(crate) fn advance_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1).max(1);
        self.sequence
    }
}

/// Physics providers report velocity/contact. The FPS locomotion capability classifies gait only;
/// presentation systems subscribe to authored events and remain free to choose any assets/effects.
#[inline]
pub(crate) fn classify_footstep_mode(
    horizontal_speed: f32,
    movement: PlayerMovementSpeeds,
    crouched: bool,
    sprint_intent: bool,
) -> FootstepLocomotionMode {
    if crouched {
        return FootstepLocomotionMode::Stealth;
    }
    let movement = movement.sanitized();
    let speed = if horizontal_speed.is_finite() {
        horizontal_speed.max(0.0)
    } else {
        0.0
    };
    if speed < movement.walk_run_threshold() {
        return FootstepLocomotionMode::Walk;
    }
    let sprint_threshold = (movement.run + movement.sprint) * 0.5;
    if sprint_intent || speed >= sprint_threshold {
        FootstepLocomotionMode::Sprint
    } else {
        FootstepLocomotionMode::Run
    }
}

#[inline]
pub(crate) fn classify_player_footstep_mode(
    world: &World,
    player: EntityId,
    horizontal_speed: f32,
) -> FootstepLocomotionMode {
    let movement = world
        .get::<PlayerMovementSpeeds>(player)
        .copied()
        .unwrap_or_default();
    let crouched = world
        .get::<PlayerStanceState>(player)
        .is_some_and(|stance| stance.current == PlayerStanceKind::Crouched);
    let sprint_intent = world
        .get::<MotorInput>(player)
        .is_some_and(|input| input.speed_mul > 1.05);
    classify_footstep_mode(horizontal_speed, movement, crouched, sprint_intent)
}

#[inline]
pub(crate) fn contact_stride(base_stride: f32, mode: FootstepLocomotionMode) -> f32 {
    let base = if base_stride.is_finite() {
        base_stride.clamp(0.25, 10.0)
    } else {
        1.4
    };
    base * match mode {
        FootstepLocomotionMode::Stealth => 0.62,
        FootstepLocomotionMode::Walk => 0.78,
        FootstepLocomotionMode::Run => 1.0,
        FootstepLocomotionMode::Sprint => 1.16,
        FootstepLocomotionMode::Land => 1.0,
    }
}

/// Read only authored/provider-neutral friction. Material names and event ids are never parsed to
/// guess whether a surface is stone, wood, metal, etc.
#[inline]
pub(crate) fn surface_friction(world: &World, ground_entity: Option<EntityId>) -> f32 {
    let authored = ground_entity.and_then(|entity| {
        world
            .get::<StaticMeshCollider>(entity)
            .map(|collider| collider.friction)
            .or_else(|| {
                world
                    .get::<PhysicsBodyDesc>(entity)
                    .map(|body| body.material.friction)
            })
    });
    authored
        .filter(|value| value.is_finite())
        .unwrap_or(0.75)
        .clamp(0.05, 1.50)
}

/// Estimate contact slip from provider-neutral motion and traction facts. This value is published
/// as event payload; the engine does not translate it into a cue/effect by itself.
#[inline]
pub(crate) fn contact_slip_ratio(
    previous_direction: Vec2,
    current_direction: Vec2,
    horizontal_speed: f32,
    friction: f32,
    slope_radians: f32,
) -> f32 {
    let previous = previous_direction.normalize_or_zero();
    let current = current_direction.normalize_or_zero();
    let turn = if previous.length_squared() > 0.5 && current.length_squared() > 0.5 {
        (1.0 - previous.dot(current).clamp(-1.0, 1.0)) * 0.5
    } else {
        0.0
    };
    let traction_loss = 1.0 - friction.clamp(0.0, 1.0);
    let speed = if horizontal_speed.is_finite() {
        horizontal_speed.max(0.0)
    } else {
        0.0
    };
    let kinetic = (speed / 8.0).clamp(0.0, 1.25);
    let slope = if slope_radians.is_finite() {
        slope_radians.sin().abs()
    } else {
        0.0
    };
    (turn * 0.62 + traction_loss * kinetic * 0.44 + traction_loss * slope * 0.28).clamp(0.0, 1.0)
}

#[inline]
pub(crate) fn landing_normal_impact_speed(
    max_downward_speed: f32,
    horizontal_velocity: Vec2,
    ground_normal: Vec3,
) -> f32 {
    let normal = if ground_normal.is_finite() {
        ground_normal.normalize_or_zero()
    } else {
        Vec3::Y
    };
    let vertical = max_downward_speed.max(0.0) * normal.y.max(0.0);
    let horizontal_into_slope =
        (-(horizontal_velocity.x * normal.x + horizontal_velocity.y * normal.z)).max(0.0);
    (vertical + horizontal_into_slope).max(0.0)
}

#[inline]
pub(crate) fn is_sharp_direction_change(previous: Vec2, current: Vec2) -> bool {
    let previous = previous.normalize_or_zero();
    let current = current.normalize_or_zero();
    if previous.length_squared() < 0.5 || current.length_squared() < 0.5 {
        return false;
    }
    previous.dot(current).clamp(-1.0, 1.0) < 0.62
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelFootContactResolution {
    pub frame: ModelFootContactFrame,
    left_surface_key: Option<u64>,
    right_surface_key: Option<u64>,
    left_slope_radians: f32,
    right_slope_radians: f32,
}

impl ModelFootContactResolution {
    pub(crate) fn surface_key(self, side: FootSide) -> Option<u64> {
        match side {
            FootSide::Left => self.left_surface_key,
            FootSide::Right => self.right_surface_key,
        }
    }

    pub(crate) fn slope_radians(self, side: FootSide) -> f32 {
        match side {
            FootSide::Left => self.left_slope_radians,
            FootSide::Right => self.right_slope_radians,
        }
    }
}

pub(crate) fn update_model_foot_contacts(
    world: &World,
    player: EntityId,
    ground: PlayerGroundState,
    contact_skin: f32,
    dt: f32,
    tracker: &mut ModelFootContactTracker,
) -> Option<ModelFootContactResolution> {
    let pose = world.get::<ModelFootPoseState>(player).copied()?;

    if !ground.grounded {
        let empty_ground = ModelFootGroundState::default();
        return Some(ModelFootContactResolution {
            frame: tracker.update_per_foot(
                pose,
                empty_ground,
                dt,
                ModelFootContactTuning::default(),
            ),
            left_surface_key: None,
            right_surface_key: None,
            left_slope_radians: 0.0,
            right_slope_radians: 0.0,
        });
    }

    // Central character grounding is the continuity fallback. Per-foot rays override this plane
    // independently when they return a valid hit, so a single ray miss cannot manufacture a
    // contact release while the character is still physically supported.
    let transform = world.get::<Transform>(player).copied()?;
    let body = world.get::<PhysicsBodyDesc>(player).copied()?;
    let vertical_extent = match body.shape.sanitized() {
        CollisionShapeDesc::Box { half_extents } => half_extents[1],
        CollisionShapeDesc::Sphere { radius } => radius,
        CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } => radius + half_height,
        CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
    };
    let fallback_plane = if ground.normal.is_finite() {
        let epsilon = (contact_skin.abs() * 0.25).clamp(0.001, 0.01);
        let point = Vec3::new(
            transform.position.x,
            transform.position.y - vertical_extent + epsilon - ground.distance.max(0.0),
            transform.position.z,
        );
        ModelGroundPlane::new(point, ground.normal)
    } else {
        ModelGroundPlane::default()
    };
    let fallback_sample = ModelFootGroundSample {
        plane: fallback_plane,
        surface_key: ground.ground_entity,
    };

    let sampled = world
        .get::<ModelFootGroundState>(player)
        .copied()
        .unwrap_or_default();
    let left = if sampled.left.plane.valid {
        sampled.left
    } else {
        fallback_sample
    };
    let right = if sampled.right.plane.valid {
        sampled.right
    } else {
        fallback_sample
    };
    let resolved_ground = ModelFootGroundState {
        revision: sampled.revision,
        left,
        right,
    };
    let slope = |sample: ModelFootGroundSample| {
        if sample.plane.valid && sample.plane.normal_world.is_finite() {
            sample.plane.normal_world.y.clamp(-1.0, 1.0).acos()
        } else {
            ground.slope_radians
        }
    };

    Some(ModelFootContactResolution {
        frame: tracker.update_per_foot(
            pose,
            resolved_ground,
            dt,
            ModelFootContactTuning::default(),
        ),
        left_surface_key: left.surface_key,
        right_surface_key: right.surface_key,
        left_slope_radians: slope(left),
        right_slope_radians: slope(right),
    })
}

pub(crate) fn phase_foot_position(
    world: &World,
    player: EntityId,
    side: FootSide,
    ground: PlayerGroundState,
    phase: FootstepPhase,
) -> [f32; 3] {
    if let Some(pose) = world.get::<ModelFootPoseState>(player) {
        let foot = match side {
            FootSide::Left => pose.left,
            FootSide::Right => pose.right,
        };
        if foot.valid && foot.position_world.is_finite() {
            let point = foot.position_world;
            return [point.x, point.y, point.z];
        }
    }
    let (position, rotation) = world
        .get::<Transform>(player)
        .map(|transform| (transform.position, transform.rotation))
        .unwrap_or((Vec3::ZERO, newengine_math::Quat::IDENTITY));
    let normal = if ground.normal.is_finite() && ground.normal.length_squared() > 0.25 {
        ground.normal.normalize_or_zero()
    } else {
        Vec3::Y
    };
    let raw_right = rotation * Vec3::X;
    let raw_forward = rotation * -Vec3::Z;
    let body_right = (raw_right - normal * raw_right.dot(normal)).normalize_or_zero();
    let body_forward = (raw_forward - normal * raw_forward.dot(normal)).normalize_or_zero();
    let body = world
        .get::<CharacterBody>(player)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let crouched = world
        .get::<PlayerStanceState>(player)
        .is_some_and(|stance| stance.current == PlayerStanceKind::Crouched);
    let half_height = if crouched {
        body.crouched_half_height
    } else {
        body.standing_half_height
    };
    let lateral = (body.radius * 0.34).clamp(0.08, 0.20) * side.sign();
    let forward = match phase {
        FootstepPhase::Scuff => (body.radius * 0.08).clamp(0.01, 0.05),
        FootstepPhase::Contact | FootstepPhase::Land => 0.0,
    };
    let mut point = Vec3::new(
        position.x,
        position.y - (half_height + body.radius) + 0.025,
        position.z,
    ) + body_right * lateral
        + body_forward * forward;
    // Keep the emitter on the local ground plane instead of leaving left/right feet at the
    // same Y on slopes. The plane anchor is the capsule sole center approximation above.
    if normal.y.abs() > 0.20 {
        let offset = point
            - Vec3::new(
                position.x,
                position.y - (half_height + body.radius) + 0.025,
                position.z,
            );
        point.y += -(normal.x * offset.x + normal.z * offset.z) / normal.y;
    }
    [point.x, point.y, point.z]
}

pub(crate) fn landing_position(
    world: &World,
    player: EntityId,
    ground: PlayerGroundState,
) -> [f32; 3] {
    let left = phase_foot_position(world, player, FootSide::Left, ground, FootstepPhase::Land);
    let right = phase_foot_position(world, player, FootSide::Right, ground, FootstepPhase::Land);
    [
        (left[0] + right[0]) * 0.5,
        (left[1] + right[1]) * 0.5,
        (left[2] + right[2]) * 0.5,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gait_classification_distinguishes_walk_run_sprint_and_stealth() {
        let movement = PlayerMovementSpeeds {
            walk: 2.0,
            run: 5.0,
            sprint: 9.0,
            crouch: 1.8,
        };
        assert_eq!(
            classify_footstep_mode(1.8, movement, false, false),
            FootstepLocomotionMode::Walk
        );
        assert_eq!(
            classify_footstep_mode(5.5, movement, false, false),
            FootstepLocomotionMode::Run
        );
        assert_eq!(
            classify_footstep_mode(5.5, movement, false, true),
            FootstepLocomotionMode::Sprint
        );
        assert_eq!(
            classify_footstep_mode(8.0, movement, true, true),
            FootstepLocomotionMode::Stealth
        );
    }

    #[test]
    fn stride_changes_with_gait() {
        let base = 1.4;
        assert!(
            contact_stride(base, FootstepLocomotionMode::Stealth)
                < contact_stride(base, FootstepLocomotionMode::Walk)
        );
        assert!(
            contact_stride(base, FootstepLocomotionMode::Walk)
                < contact_stride(base, FootstepLocomotionMode::Run)
        );
        assert!(
            contact_stride(base, FootstepLocomotionMode::Run)
                < contact_stride(base, FootstepLocomotionMode::Sprint)
        );
    }

    #[test]
    fn sharp_turn_requires_meaningful_heading_change() {
        assert!(!is_sharp_direction_change(Vec2::X, Vec2::new(0.9, 0.1)));
        assert!(is_sharp_direction_change(Vec2::X, Vec2::Y));
        assert!(!is_sharp_direction_change(Vec2::ZERO, Vec2::Y));
    }

    #[test]
    fn low_friction_turn_produces_more_slip_than_straight_high_traction_motion() {
        let stable = contact_slip_ratio(Vec2::X, Vec2::X, 5.0, 0.95, 0.0);
        let sliding = contact_slip_ratio(Vec2::X, -Vec2::X, 7.0, 0.25, 0.45);
        assert!(sliding > stable + 0.5);
    }

    #[test]
    fn landing_impact_projects_velocity_against_slope_normal() {
        let flat = landing_normal_impact_speed(6.0, Vec2::new(4.0, 0.0), Vec3::Y);
        let slope =
            landing_normal_impact_speed(6.0, Vec2::new(-4.0, 0.0), Vec3::new(0.5, 0.8660254, 0.0));
        assert!(flat > 5.9);
        assert!(slope > 5.0);
    }
}
