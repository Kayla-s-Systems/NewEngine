use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    CharacterBody, CollisionShapeDesc, PhysicsBodyDesc, PhysicsSurface, PlayerGroundState,
    PlayerMovementSpeeds, PlayerStanceKind, PlayerStanceState, StaticMeshCollider,
};
use newengine_math::{Vec2, Vec3};
use newengine_model_contact_api::{
    ModelFootContactFrame, ModelFootContactTracker, ModelFootContactTuning, ModelFootGroundSample,
    ModelFootGroundState, ModelFootPoseState, ModelGroundPlane,
};
use newengine_sim::MotorInput;
use newengine_transform::Transform;

pub(crate) const FOOTSTEP_DICTIONARY: &str = "shared/audio/footsteps/footsteps.yscd";

#[derive(Default)]
struct FootstepAudioPreloadAttempted;

/// Decode/materialize the dictionary before the first stride event. AudioRuntime keeps the
/// decoded YSCD dictionary cached, so later gait/phase entries do not re-read/re-decode it.
pub fn ensure_footstep_audio_preloaded(world: &mut World) {
    if world.resource::<FootstepAudioPreloadAttempted>().is_some() {
        return;
    }
    let _ =
        newengine_audio_client::preload_audio_cue(&newengine_audio_api::AudioCuePreloadRequest {
            cue: newengine_audio_api::SoundCueRef::new(format!("{FOOTSTEP_DICTIONARY}@stone_run")),
        });
    world.insert_resource(FootstepAudioPreloadAttempted);
}

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
    Toe,
    Lift,
    Scuff,
    Land,
}

impl FootstepPhase {
    #[inline]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Contact => "contact",
            Self::Toe => "toe",
            Self::Lift => "lift",
            Self::Scuff => "scuff",
            Self::Land => "land",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FootstepSurfaceKind {
    Dirt,
    Grass,
    Metal,
    Stone,
    Wood,
    Mud,
    Water,
    Snow,
}

impl FootstepSurfaceKind {
    #[inline]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Dirt => "dirt",
            Self::Grass => "grass",
            Self::Metal => "metal",
            Self::Stone => "stone",
            Self::Wood => "wood",
            Self::Mud => "mud",
            Self::Water => "water",
            Self::Snow => "snow",
        }
    }
}

/// Gameplay-side physical response of a surface contact. These coefficients are not owned by
/// the physics backend: they translate stable material identity + collider friction into the
/// audible mechanics of a shoe/ground contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FootstepSurfacePhysics {
    pub(crate) hardness: f32,
    pub(crate) roughness: f32,
    pub(crate) compliance: f32,
    pub(crate) wetness: f32,
    pub(crate) default_friction: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FootstepContactModulation {
    pub(crate) gain: f32,
    pub(crate) pitch: f32,
    pub(crate) stride_scale: f32,
    pub(crate) scuff_gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FootstepLandingModulation {
    pub(crate) gain: f32,
    pub(crate) pitch: f32,
    pub(crate) normal_impact_speed: f32,
}

#[inline]
pub(crate) const fn surface_physics_profile(kind: FootstepSurfaceKind) -> FootstepSurfacePhysics {
    match kind {
        FootstepSurfaceKind::Dirt => FootstepSurfacePhysics {
            hardness: 0.30,
            roughness: 0.68,
            compliance: 0.48,
            wetness: 0.10,
            default_friction: 0.82,
        },
        FootstepSurfaceKind::Grass => FootstepSurfacePhysics {
            hardness: 0.18,
            roughness: 0.78,
            compliance: 0.58,
            wetness: 0.18,
            default_friction: 0.76,
        },
        FootstepSurfaceKind::Metal => FootstepSurfacePhysics {
            hardness: 0.92,
            roughness: 0.28,
            compliance: 0.08,
            wetness: 0.02,
            default_friction: 0.62,
        },
        FootstepSurfaceKind::Stone => FootstepSurfacePhysics {
            hardness: 0.97,
            roughness: 0.48,
            compliance: 0.04,
            wetness: 0.02,
            default_friction: 0.88,
        },
        FootstepSurfaceKind::Wood => FootstepSurfacePhysics {
            hardness: 0.68,
            roughness: 0.58,
            compliance: 0.18,
            wetness: 0.03,
            default_friction: 0.72,
        },
        FootstepSurfaceKind::Mud => FootstepSurfacePhysics {
            hardness: 0.10,
            roughness: 0.82,
            compliance: 0.86,
            wetness: 0.75,
            default_friction: 0.45,
        },
        FootstepSurfaceKind::Water => FootstepSurfacePhysics {
            hardness: 0.05,
            roughness: 0.20,
            compliance: 0.95,
            wetness: 1.00,
            default_friction: 0.30,
        },
        FootstepSurfaceKind::Snow => FootstepSurfacePhysics {
            hardness: 0.16,
            roughness: 0.72,
            compliance: 0.76,
            wetness: 0.18,
            default_friction: 0.52,
        },
    }
}

#[inline]
pub(crate) fn surface_friction(
    world: &World,
    ground_entity: Option<EntityId>,
    kind: FootstepSurfaceKind,
) -> f32 {
    ground_entity
        .and_then(|entity| world.get::<StaticMeshCollider>(entity))
        .map(|collider| collider.friction)
        .filter(|value| value.is_finite())
        .unwrap_or_else(|| surface_physics_profile(kind).default_friction)
        .clamp(0.05, 1.50)
}

#[inline]
fn mode_target_speed(mode: FootstepLocomotionMode, movement: PlayerMovementSpeeds) -> f32 {
    let movement = movement.sanitized();
    match mode {
        FootstepLocomotionMode::Walk => movement.walk,
        FootstepLocomotionMode::Run => movement.run,
        FootstepLocomotionMode::Sprint => movement.sprint,
        FootstepLocomotionMode::Stealth => movement.crouch,
        FootstepLocomotionMode::Land => movement.run,
    }
    .max(0.05)
}

/// Estimate shoe slip from steering reversal, low traction, speed and slope. This is deliberately
/// derived from provider-neutral facts rather than asking the physics backend to know about audio.
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
pub(crate) fn contact_modulation(
    kind: FootstepSurfaceKind,
    mode: FootstepLocomotionMode,
    horizontal_speed: f32,
    movement: PlayerMovementSpeeds,
    friction: f32,
    slope_radians: f32,
    slip_ratio: f32,
) -> FootstepContactModulation {
    let profile = surface_physics_profile(kind);
    let target = mode_target_speed(mode, movement);
    let speed = if horizontal_speed.is_finite() {
        horizontal_speed.max(0.0)
    } else {
        0.0
    };
    let speed_ratio = (speed / target).clamp(0.35, 1.50);
    let slope = if slope_radians.is_finite() {
        slope_radians.sin().abs()
    } else {
        0.0
    };
    let mode_energy = match mode {
        FootstepLocomotionMode::Stealth => 0.82,
        FootstepLocomotionMode::Walk => 0.90,
        FootstepLocomotionMode::Run => 1.00,
        FootstepLocomotionMode::Sprint => 1.08,
        FootstepLocomotionMode::Land => 1.0,
    };
    let material =
        0.88 + profile.hardness * 0.18 + profile.roughness * 0.04 - profile.compliance * 0.08;
    let kinetic = 0.88 + speed_ratio * 0.12;
    let traction = 1.0 + slip_ratio.clamp(0.0, 1.0) * profile.roughness * 0.08;
    let gain = (mode_energy * material * kinetic * traction).clamp(0.48, 1.28);
    let pitch = (1.0 + (profile.hardness - 0.5) * 0.040 + (speed_ratio - 1.0) * 0.026
        - profile.wetness * 0.035
        - profile.compliance * 0.018
        + slope * 0.010)
        .clamp(0.90, 1.10);
    let stride_scale = (1.0 - profile.compliance * 0.10 - slope * 0.06
        + friction.clamp(0.0, 1.0) * 0.025)
        .clamp(0.78, 1.04);
    let scuff_gain = ((0.58 + 0.48 * slip_ratio.clamp(0.0, 1.0))
        * (0.78 + profile.roughness * 0.22)
        * (1.0 - profile.wetness * 0.12))
        .clamp(0.38, 1.15);
    FootstepContactModulation {
        gain,
        pitch,
        stride_scale,
        scuff_gain,
    }
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
pub(crate) fn landing_modulation(
    kind: FootstepSurfaceKind,
    normal_impact_speed: f32,
    threshold: f32,
) -> FootstepLandingModulation {
    let profile = surface_physics_profile(kind);
    let impact_gain = landing_gain(normal_impact_speed, threshold);
    let material =
        0.84 + profile.hardness * 0.22 + profile.roughness * 0.04 - profile.compliance * 0.08;
    let gain = (impact_gain * material).clamp(0.55, 1.28);
    let impact =
        ((normal_impact_speed - threshold.max(0.1)) / (threshold.max(0.1) * 2.0)).clamp(0.0, 1.0);
    let pitch =
        (0.97 + profile.hardness * 0.055 - profile.compliance * 0.030 - profile.wetness * 0.025
            + impact * 0.020)
            .clamp(0.90, 1.08);
    FootstepLandingModulation {
        gain,
        pitch,
        normal_impact_speed,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FootstepAudioAction {
    pub(crate) cue: String,
    pub(crate) position: [f32; 3],
    pub(crate) gain: f32,
    pub(crate) pitch: f32,
    pub(crate) seed: u64,
    pub(crate) phase: FootstepPhase,
    pub(crate) foot: Option<FootSide>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingFootstepAudio {
    pub(crate) remaining_seconds: f32,
    pub(crate) action: FootstepAudioAction,
}

#[derive(Clone, Debug)]
pub(crate) struct FootstepRuntimeState {
    pub(crate) next_foot: FootSide,
    pub(crate) sequence: u64,
    pub(crate) last_direction: Vec2,
    pub(crate) last_mode: Option<FootstepLocomotionMode>,
    pub(crate) was_moving: bool,
    pub(crate) scuff_cooldown: f32,
    pub(crate) pending: Vec<PendingFootstepAudio>,
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
            pending: Vec::new(),
            model_contacts: ModelFootContactTracker::default(),
        }
    }
}

impl FootstepRuntimeState {
    pub(crate) fn tick_pending(&mut self, dt: f32) -> Vec<FootstepAudioAction> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        let mut due = Vec::new();
        let mut waiting = Vec::with_capacity(self.pending.len());
        for mut pending in std::mem::take(&mut self.pending) {
            pending.remaining_seconds -= dt.max(0.0);
            if pending.remaining_seconds <= 0.0 {
                due.push(pending.action);
            } else {
                waiting.push(pending);
            }
        }
        self.pending = waiting;
        due
    }

    #[inline]
    pub(crate) fn cancel_pending(&mut self) {
        self.pending.clear();
    }

    #[inline]
    pub(crate) fn advance_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1).max(1);
        self.sequence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FootstepContactPlan {
    pub(crate) primary_cue: String,
    pub(crate) toe_cue: Option<String>,
    pub(crate) toe_delay_seconds: f32,
    pub(crate) toe_gain: f32,
    pub(crate) lift_cue: Option<String>,
    pub(crate) lift_delay_seconds: f32,
    pub(crate) lift_gain: f32,
}

/// Product-owned gait classification. Physics reports velocity/contact only; FPS policy owns
/// whether that motion is walk/run/sprint/stealth.
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

/// Gait-specific distance between consecutive foot contacts. The authored base stride is kept as
/// the run step distance; quiet movement shortens it and sprint lengthens it.
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

#[inline]
pub(crate) fn classify_surface(surface: &PhysicsSurface) -> FootstepSurfaceKind {
    let semantic = format!(
        "{} {} {}",
        surface.id, surface.footstep_event, surface.landing_event
    )
    .to_ascii_lowercase();
    if semantic.contains("water") || semantic.contains("puddle") || semantic.contains("wet") {
        FootstepSurfaceKind::Water
    } else if semantic.contains("snow") || semantic.contains("ice") {
        FootstepSurfaceKind::Snow
    } else if semantic.contains("mud") {
        FootstepSurfaceKind::Mud
    } else if semantic.contains("grass") || semantic.contains("foliage") {
        FootstepSurfaceKind::Grass
    } else if semantic.contains("wood") || semantic.contains("timber") || semantic.contains("deck")
    {
        FootstepSurfaceKind::Wood
    } else if semantic.contains("metal") || semantic.contains("steel") || semantic.contains("iron")
    {
        FootstepSurfaceKind::Metal
    } else if semantic.contains("dirt") || semantic.contains("soil") || semantic.contains("earth") {
        FootstepSurfaceKind::Dirt
    } else {
        FootstepSurfaceKind::Stone
    }
}

#[inline]
pub(crate) fn resolve_footstep_cue(
    surface: &PhysicsSurface,
    mode: FootstepLocomotionMode,
) -> String {
    let surface = classify_surface(surface);
    format!("{FOOTSTEP_DICTIONARY}@{}_{}", surface.slug(), mode.slug())
}

pub(crate) fn contact_plan(
    surface: &PhysicsSurface,
    mode: FootstepLocomotionMode,
) -> FootstepContactPlan {
    let kind = classify_surface(surface);
    let surface_slug = kind.slug();
    let toe_available = match mode {
        FootstepLocomotionMode::Walk => {
            !matches!(kind, FootstepSurfaceKind::Metal | FootstepSurfaceKind::Snow)
        }
        FootstepLocomotionMode::Stealth => true,
        FootstepLocomotionMode::Run | FootstepLocomotionMode::Sprint => {
            matches!(kind, FootstepSurfaceKind::Grass)
        }
        FootstepLocomotionMode::Land => false,
    };
    let toe_delay_seconds = match mode {
        FootstepLocomotionMode::Stealth => 0.090,
        FootstepLocomotionMode::Walk => 0.068,
        FootstepLocomotionMode::Run => 0.045,
        FootstepLocomotionMode::Sprint => 0.034,
        FootstepLocomotionMode::Land => 0.0,
    };
    let lift_available = mode == FootstepLocomotionMode::Sprint
        && matches!(
            kind,
            FootstepSurfaceKind::Dirt
                | FootstepSurfaceKind::Metal
                | FootstepSurfaceKind::Stone
                | FootstepSurfaceKind::Wood
                | FootstepSurfaceKind::Water
        );
    FootstepContactPlan {
        primary_cue: resolve_footstep_cue(surface, mode),
        toe_cue: toe_available
            .then(|| format!("{FOOTSTEP_DICTIONARY}@{}_{}_toe", surface_slug, mode.slug())),
        toe_delay_seconds,
        toe_gain: match mode {
            FootstepLocomotionMode::Stealth => 0.68,
            FootstepLocomotionMode::Walk => 0.72,
            FootstepLocomotionMode::Run => 0.76,
            FootstepLocomotionMode::Sprint => 0.80,
            FootstepLocomotionMode::Land => 1.0,
        },
        lift_cue: lift_available.then(|| format!("{FOOTSTEP_DICTIONARY}@{surface_slug}_lift")),
        lift_delay_seconds: 0.105,
        lift_gain: 0.62,
    }
}

#[inline]
pub(crate) fn scuff_cue(surface: &PhysicsSurface) -> String {
    format!(
        "{FOOTSTEP_DICTIONARY}@{}_scuff",
        classify_surface(surface).slug()
    )
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
        FootstepPhase::Toe => (body.radius * 0.22).clamp(0.04, 0.12),
        FootstepPhase::Lift => -(body.radius * 0.10).clamp(0.02, 0.06),
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

#[inline]
pub(crate) fn landing_gain(downward_speed: f32, threshold: f32) -> f32 {
    let threshold = threshold.max(0.1);
    let normalized = ((downward_speed - threshold) / (threshold * 2.0)).clamp(0.0, 1.0);
    0.78 + normalized * 0.37
}

#[inline]
pub(crate) fn phase_seed(
    player: EntityId,
    sequence: u64,
    surface: FootstepSurfaceKind,
    mode: FootstepLocomotionMode,
    phase: FootstepPhase,
    foot: Option<FootSide>,
) -> u64 {
    let mut value = player.stable_u64() ^ sequence.rotate_left(17);
    value ^= (surface as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= (mode as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (phase as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    if let Some(side) = foot {
        value ^= (side as u64 + 1).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    }
    value
}

pub(crate) fn play_locomotion_action(action: &FootstepAudioAction) {
    let mut request = newengine_audio_api::AudioCuePlayRequest::new(action.cue.clone());
    request.position = Some(action.position);
    request.gain = action.gain;
    request.pitch = action.pitch;
    request.seed = Some(action.seed);
    if let Err(error) = newengine_audio_client::play_audio_cue(&request) {
        newengine_ulog_api::ulog::warn!(
            "footstep audio gateway failed cue='{}' phase='{}' foot='{}' err='{}'",
            action.cue,
            action.phase.slug(),
            action.foot.map(FootSide::slug).unwrap_or("both"),
            error,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(id: &str, footstep: &str) -> PhysicsSurface {
        PhysicsSurface {
            id: id.to_owned(),
            footstep_event: footstep.to_owned(),
            landing_event: footstep.replace("footstep", "landing"),
        }
    }

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
            classify_footstep_mode(8.0, movement, false, false),
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
    fn contact_plan_uses_real_secondary_phase_availability() {
        let grass = surface("surface.grass", "audio.footstep.grass");
        let grass_run = contact_plan(&grass, FootstepLocomotionMode::Run);
        assert!(grass_run
            .toe_cue
            .as_deref()
            .is_some_and(|cue| cue.ends_with("@grass_run_toe")));
        assert!(grass_run.lift_cue.is_none());

        let metal = surface("surface.metal", "audio.footstep.metal");
        let metal_sprint = contact_plan(&metal, FootstepLocomotionMode::Sprint);
        assert!(metal_sprint.toe_cue.is_none());
        assert!(metal_sprint
            .lift_cue
            .as_deref()
            .is_some_and(|cue| cue.ends_with("@metal_lift")));
    }

    #[test]
    fn concrete_projects_to_tlou2_stone_family() {
        assert_eq!(
            resolve_footstep_cue(
                &surface("surface.platform", "audio.footstep.concrete"),
                FootstepLocomotionMode::Walk,
            ),
            "shared/audio/footsteps/footsteps.yscd@stone_walk"
        );
    }

    #[test]
    fn sharp_turn_requires_meaningful_heading_change() {
        assert!(!is_sharp_direction_change(Vec2::X, Vec2::new(0.9, 0.1)));
        assert!(is_sharp_direction_change(Vec2::X, Vec2::Y));
        assert!(!is_sharp_direction_change(Vec2::ZERO, Vec2::Y));
    }

    #[test]
    fn pending_phases_fire_only_after_delay() {
        let mut state = FootstepRuntimeState::default();
        state.pending.push(PendingFootstepAudio {
            remaining_seconds: 0.05,
            action: FootstepAudioAction {
                cue: "test@toe".to_owned(),
                position: [0.0; 3],
                gain: 1.0,
                pitch: 1.0,
                seed: 1,
                phase: FootstepPhase::Toe,
                foot: Some(FootSide::Left),
            },
        });
        assert!(state.tick_pending(0.02).is_empty());
        let due = state.tick_pending(0.04);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].phase, FootstepPhase::Toe);
    }

    #[test]
    fn landing_gain_scales_but_stays_bounded() {
        assert!((landing_gain(3.0, 3.0) - 0.78).abs() < 1.0e-5);
        assert!(landing_gain(6.0, 3.0) > 0.78);
        assert!(landing_gain(30.0, 3.0) <= 1.15 + f32::EPSILON);
    }

    #[test]
    fn physical_surface_profiles_separate_hard_and_compliant_contacts() {
        let stone = surface_physics_profile(FootstepSurfaceKind::Stone);
        let mud = surface_physics_profile(FootstepSurfaceKind::Mud);
        assert!(stone.hardness > mud.hardness);
        assert!(mud.compliance > stone.compliance);
        assert!(mud.wetness > stone.wetness);
    }

    #[test]
    fn low_friction_turn_produces_more_slip_than_straight_high_traction_motion() {
        let stable = contact_slip_ratio(Vec2::X, Vec2::X, 5.0, 0.95, 0.0);
        let sliding = contact_slip_ratio(Vec2::X, -Vec2::X, 7.0, 0.25, 0.45);
        assert!(sliding > stable + 0.5);
    }

    #[test]
    fn physical_modulation_makes_hard_surface_brighter_than_mud() {
        let movement = PlayerMovementSpeeds {
            walk: 2.0,
            run: 5.0,
            sprint: 9.0,
            crouch: 1.5,
        };
        let stone = contact_modulation(
            FootstepSurfaceKind::Stone,
            FootstepLocomotionMode::Run,
            5.0,
            movement,
            0.9,
            0.0,
            0.0,
        );
        let mud = contact_modulation(
            FootstepSurfaceKind::Mud,
            FootstepLocomotionMode::Run,
            5.0,
            movement,
            0.45,
            0.0,
            0.0,
        );
        assert!(stone.pitch > mud.pitch);
        assert!(mud.stride_scale < stone.stride_scale);
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
