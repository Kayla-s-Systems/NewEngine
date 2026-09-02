use newengine_math::Vec3;
use newengine_model_domain_api::MeshRenderOptions;

#[derive(Clone, Debug)]
pub struct AuthoredPlayerModelSpec {
    pub enabled: bool,
    pub source: String,
    pub properties_ref: Option<String>,
    pub texture_dictionary: Option<String>,
    pub skeleton: Option<String>,
    pub animation_slots: std::collections::BTreeMap<String, String>,
    pub animation_event_bindings: std::collections::BTreeMap<String, String>,
    pub idle_animation: Option<String>,
    pub walk_animation: Option<String>,
    pub run_animation: Option<String>,
    pub sprint_animation: Option<String>,
    pub crouch_idle_animation: Option<String>,
    pub crouch_walk_animation: Option<String>,
    pub jump_animation: Option<String>,
    pub fall_animation: Option<String>,
    pub fall_low_animation: Option<String>,
    pub fall_medium_animation: Option<String>,
    pub fall_high_animation: Option<String>,
    pub landing_soft_animation: Option<String>,
    pub landing_medium_animation: Option<String>,
    pub landing_hard_animation: Option<String>,
    pub landing_hard_run_animation: Option<String>,
    pub fall_medium_min_distance: f32,
    pub fall_high_min_distance: f32,
    pub detached_head_follow: bool,
    pub detached_head_follow_rule:
        Option<newengine_engine_runtime::gameplay::PlayerPaletteFollowRule>,
    pub eye_parent_follow: bool,
    pub eye_parent_follow_rule:
        Option<newengine_engine_runtime::gameplay::PlayerEyeParentFollowRule>,
    pub helper_pose_copies: Vec<newengine_engine_runtime::gameplay::PlayerJointCopyRule>,
    pub skin_sidecar: Option<newengine_engine_runtime::gameplay::PlayerSkinSidecarDefinition>,
    pub braid_secondary_motion:
        Option<newengine_engine_runtime::gameplay::PlayerBraidSecondaryMotionRig>,
    pub skeletal_secondary_motion:
        Option<newengine_engine_runtime::gameplay::PlayerSkeletalSecondaryMotionRig>,
    pub equipment_ready_animation: Option<String>,
    pub equipment_aim_animation: Option<String>,
    pub equipment_reload_animation: Option<String>,
    pub unarmed_ready_animation: Option<String>,
    pub unarmed_attack_animation: Option<String>,
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
    pub equipment_ready_sample_phase: f32,
    pub equipment_ready_sample_phases: std::collections::BTreeMap<String, f32>,
    pub equipment_ready_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub equipment_aim_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub equipment_reload_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub equipment_arm_ik: bool,
    pub equipment_arm_ik_rig:
        Option<newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
    pub local_offset: Vec3,
    pub yaw_offset: f32,
    pub hide_in_first_person: bool,
    pub render_options: MeshRenderOptions,
}
