use newengine_bounds::{Aabb, Bounds, Sphere};
use newengine_math::Vec3;
use newengine_procedural_noise::HeightField;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorPlayMode {
    #[default]
    Edit,
    Simulate,
    Play,
}

impl EditorPlayMode {
    #[inline]
    pub const fn is_runtime(self) -> bool {
        matches!(self, Self::Simulate | Self::Play)
    }

    #[inline]
    pub const fn runs_physics(self) -> bool {
        self.is_runtime()
    }

    #[inline]
    pub const fn wants_direct_player_control(self) -> bool {
        matches!(self, Self::Play)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollisionShape {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Default for CollisionShape {
    #[inline]
    fn default() -> Self {
        Self::Box {
            half_extents: [0.5, 0.5, 0.5],
        }
    }
}

impl CollisionShape {
    #[inline]
    pub fn local_aabb(self) -> Aabb {
        match self {
            CollisionShape::Box { half_extents } => Aabb::from_center_half_extents(
                Vec3::ZERO,
                Vec3::new(half_extents[0], half_extents[1], half_extents[2]),
            ),
            CollisionShape::Sphere { radius } => {
                Aabb::from_center_half_extents(Vec3::ZERO, Vec3::splat(radius.max(0.001)))
            }
            CollisionShape::Capsule {
                radius,
                half_height,
            } => {
                let r = radius.max(0.001);
                let hy = half_height.max(0.0) + r;
                Aabb::from_center_half_extents(Vec3::ZERO, Vec3::new(r, hy, r))
            }
        }
    }

    #[inline]
    pub fn local_sphere(self) -> Sphere {
        match self {
            CollisionShape::Box { half_extents } => {
                let he = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
                Sphere::new(Vec3::ZERO, he.length().max(0.001))
            }
            CollisionShape::Sphere { radius } => Sphere::new(Vec3::ZERO, radius.max(0.001)),
            CollisionShape::Capsule {
                radius,
                half_height,
            } => Sphere::new(Vec3::ZERO, (half_height.max(0.0) + radius.max(0.001)).max(0.001)),
        }
    }

    #[inline]
    pub fn to_bounds(self) -> Bounds {
        match self {
            CollisionShape::Sphere { .. } => Bounds::from_local_sphere(self.local_sphere()),
            _ => Bounds::from_local_aabb(self.local_aabb()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionBody {
    pub shape: CollisionShape,
    pub dynamic: bool,
    pub is_trigger: bool,
}

impl Default for CollisionBody {
    #[inline]
    fn default() -> Self {
        Self {
            shape: CollisionShape::default(),
            dynamic: false,
            is_trigger: false,
        }
    }
}

impl CollisionBody {
    #[inline]
    pub fn to_bounds(self) -> Bounds {
        self.shape.to_bounds()
    }
}

/// Stable physics component for terrain heightfield collision.
///
/// Render terrain may be streamed, rebuilt, or split into passes; physics consumes
/// this collider contract and samples the exact height surface. Coarse AABB tiles
/// remain editor/debug proxies, not the runtime source of truth.
#[derive(Clone, Debug)]
pub struct HeightfieldCollider {
    pub heightfield: Arc<HeightField>,
    pub contact_skin: f32,
}

impl HeightfieldCollider {
    #[inline]
    pub fn new(heightfield: Arc<HeightField>) -> Self {
        Self {
            heightfield,
            contact_skin: 0.08,
        }
    }

    #[inline]
    pub fn with_contact_skin(mut self, contact_skin: f32) -> Self {
        self.contact_skin = if contact_skin.is_finite() {
            contact_skin.clamp(0.0, 0.50)
        } else {
            0.08
        };
        self
    }

    #[inline]
    pub fn local_bounds(&self) -> Bounds {
        Bounds::from_local_aabb(self.heightfield.local_bounds())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerActor;

#[derive(Clone, Copy, Debug, Default)]
pub struct GameplayActor;

/// Declarative FPS runtime tuning.
///
/// The scene/profile owns these values; runtime systems only consume the resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsPlayerTuning {
    pub body_radius: f32,
    pub body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub sprint_multiplier: f32,
    pub gravity: f32,
    pub contact_skin: f32,
}

impl Default for FpsPlayerTuning {
    #[inline]
    fn default() -> Self {
        Self {
            body_radius: 0.45,
            body_half_height: 0.45,
            visual_radius: 0.45,
            visual_half_height: 0.90,
            camera_eye_height: 0.85,
            sprint_multiplier: 1.75,
            gravity: 9.81,
            contact_skin: 0.035,
        }
    }
}

impl FpsPlayerTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            body_radius: self.body_radius.clamp(0.05, 5.0),
            body_half_height: self.body_half_height.clamp(0.05, 8.0),
            visual_radius: self.visual_radius.clamp(0.05, 8.0),
            visual_half_height: self.visual_half_height.clamp(0.05, 12.0),
            camera_eye_height: self.camera_eye_height.clamp(0.05, 12.0),
            sprint_multiplier: self.sprint_multiplier.clamp(1.0, 8.0),
            gravity: self.gravity.clamp(0.0, 80.0),
            contact_skin: self.contact_skin.clamp(0.0, 0.50),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FpsDemoRules {
    pub default_status: String,
    pub pickup_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
    pub player: FpsPlayerTuning,
}

impl Default for FpsDemoRules {
    #[inline]
    fn default() -> Self {
        Self {
            default_status: "Find blue cores, avoid hazards, reach extraction.".to_string(),
            pickup_status: "Core acquired.".to_string(),
            hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_string(),
            goal_locked_status: "Beacon locked: collect all cores first.".to_string(),
            goal_complete_status: "Extraction complete. Stable runtime loop is playable.".to_string(),
            failed_progress_label: "FAILED — touch a hazard to retry scene".to_string(),
            completed_progress_label: "EXTRACTED".to_string(),
            player: FpsPlayerTuning::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoPickup {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoGoal {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoHazard {
    pub radius: f32,
}

#[cfg_attr(not(feature = "editor-ui"), allow(dead_code))]
#[derive(Clone, Debug)]
pub struct FpsDemoState {
    pub title: String,
    pub objective: String,
    pub elapsed_sec: f32,
    pub pickups_collected: u32,
    pub pickups_total: u32,
    pub completed: bool,
    pub failed: bool,
    pub status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

#[cfg_attr(not(feature = "editor-ui"), allow(dead_code))]
impl FpsDemoState {
    #[inline]
    pub fn new(pickups_total: u32) -> Self {
        Self::from_rules(
            pickups_total,
            "KAYLA FPS: Extraction Yard",
            "Collect cores and reach the extraction beacon",
            &FpsDemoRules::default(),
        )
    }

    #[inline]
    pub fn from_rules(
        pickups_total: u32,
        title: impl Into<String>,
        objective: impl Into<String>,
        rules: &FpsDemoRules,
    ) -> Self {
        Self {
            title: title.into(),
            objective: objective.into(),
            elapsed_sec: 0.0,
            pickups_collected: 0,
            pickups_total,
            completed: false,
            failed: false,
            status: rules.default_status.clone(),
            failed_progress_label: rules.failed_progress_label.clone(),
            completed_progress_label: rules.completed_progress_label.clone(),
        }
    }

    #[inline]
    pub fn progress_label(&self) -> String {
        if self.completed {
            return format!("{} in {:.1}s", self.completed_progress_label, self.elapsed_sec.max(0.0));
        }
        if self.failed {
            return self.failed_progress_label.clone();
        }
        format!(
            "Cores {}/{} · {:.1}s",
            self.pickups_collected.min(self.pickups_total),
            self.pickups_total,
            self.elapsed_sec.max(0.0)
        )
    }
}

impl Default for FpsDemoState {
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Both,
    EditorOnly,
    GameOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DisplayVisibility {
    pub mode: DisplayMode,
}

impl DisplayVisibility {
    #[inline]
    pub const fn visible_in_editor(self) -> bool {
        !matches!(self.mode, DisplayMode::GameOnly)
    }

    #[inline]
    pub const fn visible_in_game(self) -> bool {
        !matches!(self.mode, DisplayMode::EditorOnly)
    }
}
