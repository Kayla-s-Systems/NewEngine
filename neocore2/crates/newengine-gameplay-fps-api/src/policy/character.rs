#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPlayableCharacterAnimations {
    /// Authoritative project-owned animation bindings. Keys are semantic capability ids chosen
    /// by the project/runtime profile; values are arbitrary asset or graph references.
    pub slots: BTreeMap<String, String>,
    /// Legacy compatibility fields. New projects should author `slots`.
    pub idle: Option<String>,
    pub walk: Option<String>,
    pub run: Option<String>,
    pub sprint: Option<String>,
    pub crouch_idle: Option<String>,
    pub crouch_walk: Option<String>,
    pub jump: Option<String>,
    pub fall: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsJointChannelsPolicy {
    pub translation: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl FpsJointChannelsPolicy {
    #[inline]
    pub const fn any(self) -> bool {
        self.translation || self.rotation || self.scale
    }
}

impl Default for FpsJointChannelsPolicy {
    fn default() -> Self {
        Self {
            translation: false,
            rotation: true,
            scale: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsJointRotationWeightPolicy {
    pub joint: String,
    pub weight: f32,
    pub channels: FpsJointChannelsPolicy,
}

impl Default for FpsJointRotationWeightPolicy {
    fn default() -> Self {
        Self {
            joint: String::new(),
            weight: 0.0,
            channels: FpsJointChannelsPolicy::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsJointCopyPolicy {
    pub source_joint: String,
    pub target_joint: String,
    pub channels: FpsJointChannelsPolicy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPaletteFollowPolicy {
    pub driver_joint: String,
    pub follower_roots: Vec<String>,
    pub include_descendants: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsEyeParentFollowPolicy {
    pub left_joint: String,
    pub right_joint: String,
    pub parent_joint: String,
    pub preserve_bind_local: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsWeaponArmIkRigPolicy {
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsBraidSecondaryMotionRigPolicy {
    pub chain_joints: Vec<String>,
    pub head_joint: String,
    pub head_base_joint: String,
    pub upper_back_joint: String,
    pub middle_back_joint: String,
    pub lower_back_joint: String,
    pub left_shoulder_joint: String,
    pub right_shoulder_joint: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCharacterPresentationPolicy {
    /// Authoritative project-owned animation bindings for presentation capabilities.
    /// Runtime never derives paths or filenames from these keys or values.
    pub animation_slots: BTreeMap<String, String>,
    /// Compatibility reconstruction for rigs whose authored control/face branches are detached
    /// from the primary deform hierarchy. Enabled only by project-authored character data.
    pub detached_head_follow: bool,
    pub detached_head_follow_rule: Option<FpsPaletteFollowPolicy>,
    pub eye_parent_follow: bool,
    pub eye_parent_follow_rule: Option<FpsEyeParentFollowPolicy>,
    pub helper_pose_copies: Vec<FpsJointCopyPolicy>,
    pub braid_secondary_motion: Option<FpsBraidSecondaryMotionRigPolicy>,
    /// Optional upper-body equipment pose clips authored for this character rig.
    pub equipment_ready_animation: Option<String>,
    pub equipment_aim_animation: Option<String>,
    pub equipment_reload_animation: Option<String>,
    /// Character-owned bare-hand presentation. Weapon definitions never carry character clips.
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
    /// Full-body pose used while the character owns NoClip traversal.
    pub noclip_animation: Option<String>,
    /// Optional height-aware full-body fall presentation. Clip refs and thresholds are authored data.
    pub fall_low_animation: Option<String>,
    pub fall_medium_animation: Option<String>,
    pub fall_high_animation: Option<String>,
    pub landing_soft_animation: Option<String>,
    pub landing_medium_animation: Option<String>,
    pub landing_hard_animation: Option<String>,
    pub landing_hard_run_animation: Option<String>,
    pub fall_medium_min_distance: f32,
    pub fall_high_min_distance: f32,
    pub equipment_ready_sample_phase: f32,
    pub equipment_ready_rotation_weights: Vec<FpsJointRotationWeightPolicy>,
    pub equipment_aim_rotation_weights: Vec<FpsJointRotationWeightPolicy>,
    pub equipment_reload_rotation_weights: Vec<FpsJointRotationWeightPolicy>,
    /// Allows the generic two-arm equipment IK stage only when the project authors an explicit rig contract.
    pub equipment_arm_ik: bool,
    pub equipment_arm_ik_rig: Option<FpsWeaponArmIkRigPolicy>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPlayableCharacterPolicy {
    pub id: String,
    /// Project-owned grouping label shown by UI. The runtime does not interpret it.
    pub family: String,
    pub display_name: String,
    pub subtitle: String,
    pub rig_label: String,
    pub source_provenance: String,
    /// Alias tokens accepted after `game.character.select.` for save/action compatibility.
    pub aliases: Vec<String>,
    pub runtime_ready: bool,
    pub runtime_model_ref: Option<String>,
    pub properties_ref: Option<String>,
    pub texture_dictionary: Option<String>,
    pub skeleton_ref: Option<String>,
    pub animations: FpsPlayableCharacterAnimations,
    pub presentation: FpsCharacterPresentationPolicy,
    pub target_height: f32,
    pub yaw_offset: f32,
    pub hide_in_first_person: bool,
}

impl Default for FpsPlayableCharacterPolicy {
    fn default() -> Self {
        Self {
            id: String::new(),
            family: String::new(),
            display_name: String::new(),
            subtitle: String::new(),
            rig_label: String::new(),
            source_provenance: String::new(),
            aliases: Vec::new(),
            runtime_ready: false,
            runtime_model_ref: None,
            properties_ref: None,
            texture_dictionary: None,
            skeleton_ref: None,
            animations: FpsPlayableCharacterAnimations::default(),
            presentation: FpsCharacterPresentationPolicy::default(),
            target_height: 0.0,
            yaw_offset: 0.0,
            hide_in_first_person: false,
        }
    }
}

impl FpsPlayableCharacterPolicy {
    fn validate_menu_entry(&self) -> Result<(), String> {
        for (label, value) in [
            ("id", self.id.as_str()),
            ("family", self.family.as_str()),
            ("display_name", self.display_name.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS playable character '{label}' must not be empty"
                ));
            }
        }
        for alias in &self.aliases {
            let alias = alias.trim();
            if alias.is_empty()
                || alias.contains('@')
                || alias.contains('/')
                || alias.contains('\\')
                || alias.starts_with("game.character.select.")
            {
                return Err(format!(
                    "FPS playable character alias must be a stable action token, got '{alias}'"
                ));
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        self.validate_menu_entry()?;
        if !self.yaw_offset.is_finite() {
            return Err(format!(
                "FPS playable character '{}' yaw_offset must be finite",
                self.id
            ));
        }
        if self.runtime_ready {
            let source = self.runtime_model_ref.as_deref().unwrap_or_default().trim();
            if source.is_empty() {
                return Err(format!(
                    "runtime-ready FPS playable character '{}' requires runtime_model_ref",
                    self.id
                ));
            }
            if !self.target_height.is_finite() || !(0.1..=10.0).contains(&self.target_height) {
                return Err(format!(
                    "runtime-ready FPS playable character '{}' target_height must be finite in [0.1, 10.0]",
                    self.id
                ));
            }
        }
        for (slot, reference) in &self.animations.slots {
            let slot = slot.trim();
            let reference = reference.trim();
            if slot.is_empty() || slot.len() > 256 || slot.chars().any(char::is_control) {
                return Err(format!(
                    "FPS playable character '{}' has invalid animation slot id '{}'",
                    self.id, slot
                ));
            }
            if reference.is_empty() {
                return Err(format!(
                    "FPS playable character '{}' animation slot '{}' must not be blank",
                    self.id, slot
                ));
            }
        }

        // Compatibility-only fixed fields are opaque authored refs. There is deliberately no
        // directory, extension, filename or character-owner convention here.
        for (label, value) in [
            ("idle", self.animations.idle.as_deref()),
            ("walk", self.animations.walk.as_deref()),
            ("run", self.animations.run.as_deref()),
            ("sprint", self.animations.sprint.as_deref()),
            ("crouch_idle", self.animations.crouch_idle.as_deref()),
            ("crouch_walk", self.animations.crouch_walk.as_deref()),
            ("jump", self.animations.jump.as_deref()),
            ("fall", self.animations.fall.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(format!(
                    "FPS playable character '{}' legacy animation '{label}' must not be blank",
                    self.id
                ));
            }
        }

        // Presentation is a set of optional capabilities attached after the entity/model/skeleton
        // boundary. Missing or malformed presentation data must never invalidate the character
        // catalog or prevent the visual entity from existing. Runtime feature binders diagnose
        // and disable individual capabilities instead.
        Ok(())
    }
}
