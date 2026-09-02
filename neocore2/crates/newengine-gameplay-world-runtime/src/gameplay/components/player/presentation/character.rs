/// Verified authored animation capabilities for the currently bound playable character.
///
/// The component is published by a character presentation provider only after its clips have
/// decoded and bound to the active skeleton. Absence of this component means the generic gameplay
/// runtime has no presentation-specific restrictions; a present component is authoritative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerAuthoredAnimationCapabilities {
    pub unarmed_ready: bool,
    pub unarmed_attack: bool,
    pub equipment_ready: bool,
    pub equipment_aim: bool,
    pub equipment_reload: bool,
    pub noclip: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerStanceKind {
    #[default]
    Standing,
    Crouched,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStanceState {
    pub current: PlayerStanceKind,
    pub stand_requested: bool,
    pub stand_blocked: bool,
    pub current_eye_height: f32,
    pub target_eye_height: f32,
    pub last_transition_tick: u64,
}

impl PlayerStanceState {
    #[inline]
    pub const fn standing(eye_height: f32) -> Self {
        Self {
            current: PlayerStanceKind::Standing,
            stand_requested: false,
            stand_blocked: false,
            current_eye_height: eye_height,
            target_eye_height: eye_height,
            last_transition_tick: 0,
        }
    }
}

impl Default for PlayerStanceState {
    fn default() -> Self {
        Self::standing(CharacterBody::default().standing_eye_height)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerJointRotationWeight {
    pub joint: String,
    pub weight: f32,
    pub channels: PlayerJointChannels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerJointChannels {
    pub translation: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl PlayerJointChannels {
    #[inline]
    pub const fn rotation_only() -> Self {
        Self {
            translation: false,
            rotation: true,
            scale: false,
        }
    }

    #[inline]
    pub const fn translation_rotation() -> Self {
        Self {
            translation: true,
            rotation: true,
            scale: false,
        }
    }

    #[inline]
    pub const fn all() -> Self {
        Self {
            translation: true,
            rotation: true,
            scale: true,
        }
    }

    #[inline]
    pub const fn any(self) -> bool {
        self.translation || self.rotation || self.scale
    }
}

impl Default for PlayerJointChannels {
    fn default() -> Self {
        Self::rotation_only()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerJointCopyRule {
    pub source_joint: String,
    pub target_joint: String,
    pub channels: PlayerJointChannels,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPaletteFollowRule {
    pub driver_joint: String,
    pub follower_roots: Vec<String>,
    pub include_descendants: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerEyeParentFollowRule {
    pub left_joint: String,
    pub right_joint: String,
    pub parent_joint: String,
    pub preserve_bind_local: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerWeaponArmIkRigDefinition {
    pub chest: String,
    pub right_shoulder: String,
    pub right_elbow: String,
    pub right_wrist: String,
    pub right_palm: String,
    pub right_prop_attachment: Option<String>,
    pub left_shoulder: String,
    pub left_elbow: String,
    pub left_wrist: String,
    pub left_palm: String,
    pub left_prop_attachment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerBraidSecondaryMotionRig {
    pub chain_joints: Vec<String>,
    pub head_joint: String,
    pub head_base_joint: String,
    pub upper_back_joint: String,
    pub middle_back_joint: String,
    pub lower_back_joint: String,
    pub left_shoulder_joint: String,
    pub right_shoulder_joint: String,
}

/// Collision projection policy for a project-authored skeletal secondary-motion collider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerSecondaryMotionColliderMode {
    #[default]
    Exterior,
    OneSidedBack,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSecondaryMotionParticle {
    /// Rest position in the model source space declared by the bound drawable.
    pub rest_position: [f32; 3],
    pub mobility: f32,
    pub follow: f32,
    pub inertia: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSecondaryMotionEdge {
    pub a: usize,
    pub b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSecondaryMotionBend {
    pub indices: [usize; 4],
    pub weights: [f32; 4],
    pub geometry_scale: f32,
    pub rest_scalar: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSecondaryMotionCapsule {
    pub joint: String,
    pub mode: PlayerSecondaryMotionColliderMode,
    /// Endpoints in the source/model authoring space. Runtime canonicalizes them through
    /// the drawable's source-to-model transform before binding them to the skeleton.
    pub source_a: [f32; 3],
    pub source_b: [f32; 3],
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSecondaryMotionOrientedBox {
    pub joint: String,
    pub mode: PlayerSecondaryMotionColliderMode,
    pub source_center: [f32; 3],
    pub source_axes: [[f32; 3]; 3],
    pub half_extents: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerSecondaryMotionTuning {
    pub teleport_reset_distance: f32,
    pub teleport_reset_quat_dot: f32,
    pub back_clearance: f32,
    pub solver_substeps: u8,
    pub solver_iterations: u8,
    pub max_root_acceleration: f32,
    pub gravity_scale: f32,
    pub inertia_scale: f32,
    pub velocity_damping: f32,
    pub collision_margin: f32,
    pub follow_scale: f32,
    pub stretch_reference_stiffness: f32,
    pub bend_reference_stiffness: f32,
    pub tunnel_depth: f32,
}

/// Fully project-authored skeletal secondary-motion contract. The runtime owns only the solver;
/// character identity, topology, constraint values and collision geometry are content.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerSkeletalSecondaryMotionRig {
    pub chain_joints: Vec<String>,
    pub dynamic_start: usize,
    pub particles: Vec<PlayerSecondaryMotionParticle>,
    pub edges: Vec<PlayerSecondaryMotionEdge>,
    pub bends: Vec<PlayerSecondaryMotionBend>,
    pub centerline_pairs: Vec<[usize; 2]>,
    pub collision_capsules: Vec<PlayerSecondaryMotionCapsule>,
    pub collision_boxes: Vec<PlayerSecondaryMotionOrientedBox>,
    pub tuning: PlayerSecondaryMotionTuning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSkinSidecarDefinition {
    pub model: String,
    pub skeleton: String,
    /// Exact authored namespace suffix carried by every joint in the auxiliary skeleton.
    /// Runtime removes only this literal suffix before attempting an exact master-joint match.
    pub joint_name_suffix: String,
    /// Exact prefix for sidecar-local joints that intentionally have no master counterpart
    /// (for example authored cloth simulation joints). Any other unresolved joint is rejected.
    pub local_joint_prefix: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerCharacterPresentation {
    /// Project-authored animation capability bindings. The engine treats both key and value as
    /// opaque semantic data and never constructs an asset path from either.
    pub animation_slots: std::collections::BTreeMap<String, String>,
    /// Project-authored semantic event -> animation slot/controller subscription table.
    pub animation_event_bindings: std::collections::BTreeMap<String, String>,
    pub detached_head_follow: bool,
    pub detached_head_follow_rule: Option<PlayerPaletteFollowRule>,
    pub eye_parent_follow: bool,
    pub eye_parent_follow_rule: Option<PlayerEyeParentFollowRule>,
    pub helper_pose_copies: Vec<PlayerJointCopyRule>,
    pub skin_sidecar: Option<PlayerSkinSidecarDefinition>,
    /// Legacy compatibility DTO. New project content must author `skeletal_secondary_motion`.
    pub braid_secondary_motion: Option<PlayerBraidSecondaryMotionRig>,
    pub skeletal_secondary_motion: Option<PlayerSkeletalSecondaryMotionRig>,
    pub equipment_ready_animation: Option<String>,
    pub equipment_aim_animation: Option<String>,
    pub equipment_reload_animation: Option<String>,
    pub unarmed_ready_animation: Option<String>,
    pub unarmed_attack_animation: Option<String>,
    pub noclip_animation: Option<String>,
    /// Optional authored turn-in-place clips. These are full-body steps; stationary mouse yaw never
    /// rotates the world root directly. Runtime selects the nearest signed angle.
    pub turn_45_left_animation: Option<String>,
    pub turn_45_right_animation: Option<String>,
    pub turn_90_left_animation: Option<String>,
    pub turn_90_right_animation: Option<String>,
    pub turn_135_left_animation: Option<String>,
    pub turn_135_right_animation: Option<String>,
    pub turn_180_left_animation: Option<String>,
    pub turn_180_right_animation: Option<String>,
    /// Optional full-body fall clips selected from the runtime fall-distance signal.
    pub fall_low_animation: Option<String>,
    pub fall_medium_animation: Option<String>,
    pub fall_high_animation: Option<String>,
    /// Optional character-native landing response clips. These run after `FallEnded`, not while airborne.
    pub landing_soft_animation: Option<String>,
    pub landing_medium_animation: Option<String>,
    pub landing_hard_animation: Option<String>,
    pub landing_hard_run_animation: Option<String>,
    /// Project-authored distance thresholds measured from the airborne trajectory apex.
    pub fall_medium_min_distance: f32,
    pub fall_high_min_distance: f32,
    pub equipment_ready_sample_phase: f32,
    /// Optional per-equipment-family READY sample phases. Keys are opaque normalized weapon-class
    /// ids (`pistol`, `knife`, `long_gun`, ...); runtime never enumerates them.
    pub equipment_ready_sample_phases: std::collections::BTreeMap<String, f32>,
    pub equipment_ready_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_aim_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_reload_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_arm_ik: bool,
    pub equipment_arm_ik_rig: Option<PlayerWeaponArmIkRigDefinition>,
}

impl Default for PlayerCharacterPresentation {
    fn default() -> Self {
        Self {
            animation_slots: std::collections::BTreeMap::new(),
            animation_event_bindings: std::collections::BTreeMap::new(),
            detached_head_follow: false,
            detached_head_follow_rule: None,
            eye_parent_follow: false,
            eye_parent_follow_rule: None,
            helper_pose_copies: Vec::new(),
            skin_sidecar: None,
            braid_secondary_motion: None,
            skeletal_secondary_motion: None,
            equipment_ready_animation: None,
            equipment_aim_animation: None,
            equipment_reload_animation: None,
            unarmed_ready_animation: None,
            unarmed_attack_animation: None,
            noclip_animation: None,
            turn_45_left_animation: None,
            turn_45_right_animation: None,
            turn_90_left_animation: None,
            turn_90_right_animation: None,
            turn_135_left_animation: None,
            turn_135_right_animation: None,
            turn_180_left_animation: None,
            turn_180_right_animation: None,
            fall_low_animation: None,
            fall_medium_animation: None,
            fall_high_animation: None,
            landing_soft_animation: None,
            landing_medium_animation: None,
            landing_hard_animation: None,
            landing_hard_run_animation: None,
            fall_medium_min_distance: 0.0,
            fall_high_min_distance: 0.0,
            equipment_ready_sample_phase: 0.0,
            equipment_ready_sample_phases: std::collections::BTreeMap::new(),
            equipment_ready_rotation_weights: Vec::new(),
            equipment_aim_rotation_weights: Vec::new(),
            equipment_reload_rotation_weights: Vec::new(),
            equipment_arm_ik: false,
            equipment_arm_ik_rig: None,
        }
    }
}
