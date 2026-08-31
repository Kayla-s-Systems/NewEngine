#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use newengine_gameplay_script_api::GameplayCommandBuffer;
use serde::{Deserialize, Serialize};

pub const FPS_GAMEPLAY_POLICY_SCHEMA: &str = "newengine.gameplay.fps.policy.v1";
pub const FPS_GAMEPLAY_POLICY_VERSION: u32 = 1;

pub const FPS_CHARACTER_MENU_POLICY_SCHEMA: &str = "newengine.gameplay.fps.character_menu.v1";
pub const FPS_CHARACTER_MENU_POLICY_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCharacterMenuPolicySnapshot {
    pub schema: String,
    pub version: u32,
    /// Semantic action consumed by the menu. Physical key mapping remains input-profile owned.
    pub toggle_action: String,
    pub title: String,
    /// Shared fallback catalog. A non-empty project `characters` catalog remains authoritative.
    pub characters: Vec<FpsPlayableCharacterPolicy>,
}

impl Default for FpsCharacterMenuPolicySnapshot {
    fn default() -> Self {
        Self {
            schema: FPS_CHARACTER_MENU_POLICY_SCHEMA.to_owned(),
            version: FPS_CHARACTER_MENU_POLICY_VERSION,
            toggle_action: crate::action::CHARACTER_SELECT_TOGGLE.to_owned(),
            title: "Character".to_owned(),
            characters: Vec::new(),
        }
    }
}

impl FpsCharacterMenuPolicySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FPS_CHARACTER_MENU_POLICY_SCHEMA {
            return Err(format!(
                "FPS character-menu policy schema mismatch: expected '{}' got '{}'",
                FPS_CHARACTER_MENU_POLICY_SCHEMA, self.schema
            ));
        }
        if self.version != FPS_CHARACTER_MENU_POLICY_VERSION {
            return Err(format!(
                "FPS character-menu policy version mismatch: expected {} got {}",
                FPS_CHARACTER_MENU_POLICY_VERSION, self.version
            ));
        }
        validate_action_id("character_menu.toggle_action", &self.toggle_action)?;
        if self.title.trim().is_empty() || self.title.len() > 96 {
            return Err("FPS character-menu title must contain 1..=96 bytes".to_owned());
        }
        let mut ids = BTreeSet::new();
        let mut aliases = BTreeSet::new();
        for character in &self.characters {
            character.validate_menu_entry()?;
            let id = character.id.trim().to_ascii_lowercase();
            if !ids.insert(id.clone()) {
                return Err(format!(
                    "duplicate Shared FPS character-menu character id '{}'",
                    character.id
                ));
            }
            for alias in &character.aliases {
                let alias = alias.trim().to_ascii_lowercase();
                if alias == id || !aliases.insert(alias.clone()) {
                    return Err(format!(
                        "duplicate/ambiguous Shared FPS character-menu alias '{}'",
                        alias
                    ));
                }
            }
        }
        Ok(())
    }
}

pub trait FpsCharacterMenuPolicyProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load_snapshot(&self) -> Result<Arc<FpsCharacterMenuPolicySnapshot>, String>;
}

fn validate_action_id(label: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(format!("{label} must contain 1..=128 bytes"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        return Err(format!(
            "{label} contains unsupported characters: '{value}'"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsGameplayPolicySnapshot {
    pub schema: String,
    pub version: u32,
    /// Authored item/weapon/loadout package. The FPS content compiler owns the
    /// conversion from this DTO into generic engine inventory components.
    pub content: serde_json::Value,
    pub required_content: FpsRequiredContentPolicy,
    /// Project-authored playable character catalog. The FPS runtime only applies these descriptors.
    pub characters: Vec<FpsPlayableCharacterPolicy>,
    pub player: FpsPlayerPolicy,
    pub combat: FpsCombatPolicy,
    pub mission: FpsMissionPolicy,
    /// Generic project-owned event routing.
    pub event_subscriptions: Vec<FpsProjectEventSubscription>,
    pub callbacks: FpsCallbackExports,
}

impl Default for FpsGameplayPolicySnapshot {
    fn default() -> Self {
        Self {
            schema: FPS_GAMEPLAY_POLICY_SCHEMA.to_owned(),
            version: FPS_GAMEPLAY_POLICY_VERSION,
            content: serde_json::json!({
                "schema": "newengine.items.package.v1",
                "version": 1,
                "items": [],
                "loadouts": []
            }),
            required_content: FpsRequiredContentPolicy::default(),
            characters: Vec::new(),
            player: FpsPlayerPolicy::default(),
            combat: FpsCombatPolicy::default(),
            mission: FpsMissionPolicy::default(),
            event_subscriptions: Vec::new(),
            callbacks: FpsCallbackExports::default(),
        }
    }
}

impl FpsGameplayPolicySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FPS_GAMEPLAY_POLICY_SCHEMA {
            return Err(format!(
                "FPS gameplay policy schema mismatch: expected '{}' got '{}'",
                FPS_GAMEPLAY_POLICY_SCHEMA, self.schema
            ));
        }
        if self.version != FPS_GAMEPLAY_POLICY_VERSION {
            return Err(format!(
                "FPS gameplay policy version mismatch: expected {} got {}",
                FPS_GAMEPLAY_POLICY_VERSION, self.version
            ));
        }
        if !self.content.is_object() {
            return Err("FPS gameplay policy content must be an item-package object".to_owned());
        }
        self.required_content.validate()?;
        let mut character_ids = BTreeSet::new();
        let mut character_aliases = BTreeSet::new();
        for character in &self.characters {
            character.validate()?;
            let id_key = character.id.trim().to_ascii_lowercase();
            if !character_ids.insert(id_key.clone()) {
                return Err(format!(
                    "duplicate FPS playable character id '{}'",
                    character.id
                ));
            }
            for alias in &character.aliases {
                let alias_key = alias.trim().to_ascii_lowercase();
                if alias_key == id_key || !character_aliases.insert(alias_key.clone()) {
                    return Err(format!(
                        "duplicate/ambiguous FPS playable character alias '{}'",
                        alias
                    ));
                }
            }
        }
        self.player.validate()?;
        self.combat.validate()?;
        self.mission.validate()?;
        let mut subscriptions = BTreeSet::new();
        for subscription in &self.event_subscriptions {
            subscription.validate()?;
            let key = (
                subscription.event.trim().to_ascii_lowercase(),
                subscription.operation.trim().to_ascii_lowercase(),
            );
            if !subscriptions.insert(key) {
                return Err(format!(
                    "duplicate FPS project event subscription event='{}' operation='{}'",
                    subscription.event, subscription.operation
                ));
            }
        }
        self.callbacks.validate()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsRequiredContentPolicy {
    pub default_loadout: String,
    pub primary_weapon: String,
    pub primary_ammo: String,
    pub medkit: String,
}

impl Default for FpsRequiredContentPolicy {
    fn default() -> Self {
        Self {
            default_loadout: String::new(),
            primary_weapon: String::new(),
            primary_ammo: String::new(),
            medkit: String::new(),
        }
    }
}

impl FpsRequiredContentPolicy {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("default_loadout", &self.default_loadout),
            ("primary_weapon", &self.primary_weapon),
            ("primary_ammo", &self.primary_ammo),
            ("medkit", &self.medkit),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS required content id '{label}' must not be empty"
                ));
            }
        }
        Ok(())
    }
}

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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPlayerPolicy {
    pub allow_jump: bool,
    pub allow_crouch: bool,
    pub allow_sprint: bool,
    pub allow_interact: bool,
    pub allow_projectile_launch: bool,
}

impl Default for FpsPlayerPolicy {
    fn default() -> Self {
        Self {
            allow_jump: true,
            allow_crouch: true,
            allow_sprint: true,
            allow_interact: true,
            allow_projectile_launch: false,
        }
    }
}

impl FpsPlayerPolicy {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCombatPolicy {
    pub allow_fire: bool,
    pub allow_melee: bool,
    pub allow_reload: bool,
    pub damage_multiplier: f32,
    pub interaction_range_multiplier: f32,
}

impl Default for FpsCombatPolicy {
    fn default() -> Self {
        Self {
            allow_fire: true,
            allow_melee: true,
            allow_reload: true,
            damage_multiplier: 1.0,
            interaction_range_multiplier: 1.0,
        }
    }
}

impl FpsCombatPolicy {
    fn validate(&self) -> Result<(), String> {
        validate_finite_range(
            "combat.damage_multiplier",
            self.damage_multiplier,
            0.0,
            100.0,
        )?;
        validate_finite_range(
            "combat.interaction_range_multiplier",
            self.interaction_range_multiplier,
            0.0,
            10.0,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsMissionStateMachinePolicy {
    pub enabled: bool,
    pub instance_id: String,
    pub machine_id: String,
    pub initial_state: String,
    pub activate_event: String,
}

impl Default for FpsMissionStateMachinePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            instance_id: String::new(),
            machine_id: String::new(),
            initial_state: String::new(),
            activate_event: String::new(),
        }
    }
}

impl FpsMissionStateMachinePolicy {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        for (label, value) in [
            ("instance_id", &self.instance_id),
            ("machine_id", &self.machine_id),
            ("initial_state", &self.initial_state),
            ("activate_event", &self.activate_event),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS mission state-machine '{label}' must not be empty when enabled"
                ));
            }
            if value.contains('@') || value.contains('\\') {
                return Err(format!(
                    "FPS mission state-machine '{label}' must be a stable authored id/event, got '{value}'"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsMissionPolicy {
    pub require_pickups: bool,
    pub require_targets: bool,
    pub hazard_fails: bool,
    pub goal_requires_objectives: bool,
    pub state_machine: FpsMissionStateMachinePolicy,
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

impl Default for FpsMissionPolicy {
    fn default() -> Self {
        Self {
            require_pickups: false,
            require_targets: false,
            hazard_fails: false,
            goal_requires_objectives: false,
            state_machine: FpsMissionStateMachinePolicy::default(),
            default_status: String::new(),
            pickup_status: String::new(),
            target_status: String::new(),
            hazard_status: String::new(),
            goal_locked_status: String::new(),
            goal_complete_status: String::new(),
            failed_progress_label: String::new(),
            completed_progress_label: String::new(),
        }
    }
}

impl FpsMissionPolicy {
    fn validate(&self) -> Result<(), String> {
        self.state_machine.validate()?;
        for (label, value) in [
            ("default_status", &self.default_status),
            ("pickup_status", &self.pickup_status),
            ("target_status", &self.target_status),
            ("hazard_status", &self.hazard_status),
            ("goal_locked_status", &self.goal_locked_status),
            ("goal_complete_status", &self.goal_complete_status),
            ("failed_progress_label", &self.failed_progress_label),
            ("completed_progress_label", &self.completed_progress_label),
        ] {
            if value.trim().is_empty() {
                return Err(format!("FPS mission policy '{label}' must not be empty"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct FpsProjectEventSubscription {
    pub event: String,
    pub operation: String,
}

impl FpsProjectEventSubscription {
    pub fn validate(&self) -> Result<(), String> {
        let event = self.event.trim();
        if event.is_empty() || event.len() > 256 || event.chars().any(char::is_control) {
            return Err(format!(
                "invalid FPS project event subscription id '{}'",
                self.event
            ));
        }
        let wildcard_count = event.matches('*').count();
        let allowed_wildcards = usize::from(event.ends_with('*'));
        if wildcard_count > allowed_wildcards {
            return Err(format!(
                "subscription wildcard is only allowed as trailing '*': '{}'",
                self.event
            ));
        }
        let operation = self.operation.trim();
        if operation.is_empty() || operation.len() > 256 {
            return Err(
                "FPS project event subscription operation must contain 1..=256 bytes".to_owned(),
            );
        }
        if operation.contains('@') || operation.contains('/') || operation.contains('\\') {
            return Err(format!(
                "subscription operation must be a name, not selector/path: '{}'",
                self.operation
            ));
        }
        Ok(())
    }
    pub fn matches(&self, event_id: &str) -> bool {
        let pattern = self.event.trim();
        if let Some(prefix) = pattern.strip_suffix('*') {
            event_id
                .to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
        } else {
            pattern.eq_ignore_ascii_case(event_id)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCallbackExports {
    pub interaction: String,
    pub hit: String,
    pub mission_event: String,
}

impl Default for FpsCallbackExports {
    fn default() -> Self {
        Self {
            interaction: String::new(),
            hit: String::new(),
            mission_event: String::new(),
        }
    }
}

impl FpsCallbackExports {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("interaction", &self.interaction),
            ("hit", &self.hit),
            ("mission_event", &self.mission_event),
        ] {
            if value.trim().is_empty() {
                continue;
            }
            if value.contains('@') || value.contains('/') || value.contains('\\') {
                return Err(format!(
                    "FPS callback export '{label}' must be an operation name, not a script selector/path: '{value}'"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FpsPolicyEvent {
    Project {
        id: String,
        source: Option<u64>,
        payload: serde_json::Value,
    },
    Interaction {
        player: u64,
        target: u64,
        prompt: String,
        fixed_tick: u64,
        point: [f32; 3],
    },
    Hit {
        shooter: u64,
        /// Concrete inventory weapon instance captured when the shot was authored.
        weapon_instance_id: u64,
        target: Option<u64>,
        shot_sequence: u64,
        base_damage: f32,
        fixed_tick: u64,
        point: [f32; 3],
        normal: [f32; 3],
    },
    Mission {
        pickups_collected: u32,
        pickups_total: u32,
        targets_destroyed: u32,
        targets_total: u32,
        collected_delta: u32,
        destroyed_delta: u32,
        hit_hazard: bool,
        reached_goal: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPolicyDecision {
    pub allow_default: bool,
    pub collect_item: Option<bool>,
    pub damage_multiplier: f32,
    pub completed: Option<bool>,
    pub failed: Option<bool>,
    pub status: Option<String>,
    pub commands: GameplayCommandBuffer,
}

impl Default for FpsPolicyDecision {
    fn default() -> Self {
        Self {
            allow_default: true,
            collect_item: None,
            damage_multiplier: 1.0,
            completed: None,
            failed: None,
            status: None,
            commands: GameplayCommandBuffer::default(),
        }
    }
}

impl FpsPolicyDecision {
    pub fn validate(&self) -> Result<(), String> {
        validate_finite_range(
            "decision.damage_multiplier",
            self.damage_multiplier,
            0.0,
            100.0,
        )?;
        if self.status.as_ref().is_some_and(|value| value.len() > 4096) {
            return Err("FPS policy callback status exceeds 4096 bytes".to_owned());
        }
        if !self.commands.commands.is_empty() {
            self.commands.validate_envelope(64)?;
        }
        Ok(())
    }
}

pub trait FpsGameplayPolicyProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load_snapshot(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String>;
    fn invoke_event(
        &self,
        export: &str,
        event: &FpsPolicyEvent,
    ) -> Result<FpsPolicyDecision, String>;
}

fn validate_finite_range(label: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!(
            "FPS policy '{label}' must be finite in [{min}, {max}], got {value}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_schema_only_and_requires_project_authored_content() {
        let policy = FpsGameplayPolicySnapshot::default();
        assert_eq!(policy.schema, FPS_GAMEPLAY_POLICY_SCHEMA);
        assert!(policy.characters.is_empty());
        assert!(policy.validate().is_err());
    }

    #[test]
    fn character_animation_bindings_are_project_defined_not_directory_owned() {
        let character = FpsPlayableCharacterPolicy {
            id: "project_character".to_owned(),
            family: "Project".to_owned(),
            display_name: "Project Character".to_owned(),
            runtime_ready: true,
            runtime_model_ref: Some("arbitrary/storage/avatar.asset@body".to_owned()),
            skeleton_ref: Some("rigs/shared/runtime.asset@skeleton".to_owned()),
            animations: FpsPlayableCharacterAnimations {
                slots: BTreeMap::from([
                    (
                        "locomotion.rest".to_owned(),
                        "motion/banks/a.asset@rest".to_owned(),
                    ),
                    (
                        "project.inspect.head".to_owned(),
                        "anywhere/custom.bin@head_turn".to_owned(),
                    ),
                ]),
                idle: Some("legacy/location/also_allowed.asset@idle".to_owned()),
                ..FpsPlayableCharacterAnimations::default()
            },
            target_height: 1.70,
            ..FpsPlayableCharacterPolicy::default()
        };
        character.validate().expect(
            "animation references are opaque project-authored bindings, not directory ownership",
        );
    }

    #[test]
    fn arbitrary_animation_slot_rejects_blank_binding_but_not_custom_names() {
        let mut character = FpsPlayableCharacterPolicy {
            id: "custom".to_owned(),
            family: "Project".to_owned(),
            display_name: "Custom".to_owned(),
            animations: FpsPlayableCharacterAnimations {
                slots: BTreeMap::from([(
                    "my.gameplay.mode.experimental".to_owned(),
                    "custom/protocol/ref#42".to_owned(),
                )]),
                ..FpsPlayableCharacterAnimations::default()
            },
            ..FpsPlayableCharacterPolicy::default()
        };
        character
            .validate()
            .expect("custom slot names and refs are project data");
        character
            .animations
            .slots
            .insert("broken".to_owned(), "   ".to_owned());
        assert!(character.validate().is_err());
    }

    #[test]
    fn character_menu_does_not_require_equipment_ik_rig() {
        let character = FpsPlayableCharacterPolicy {
            id: "generic_entity".to_owned(),
            family: "Test".to_owned(),
            display_name: "Generic Entity".to_owned(),
            presentation: FpsCharacterPresentationPolicy {
                equipment_arm_ik: true,
                equipment_arm_ik_rig: None,
                ..FpsCharacterPresentationPolicy::default()
            },
            ..FpsPlayableCharacterPolicy::default()
        };
        let policy = FpsCharacterMenuPolicySnapshot {
            characters: vec![character],
            ..FpsCharacterMenuPolicySnapshot::default()
        };
        policy
            .validate()
            .expect("optional equipment IK must not invalidate Character Menu");
    }

    #[test]
    fn runtime_character_admission_does_not_require_equipment_ik_rig() {
        let character = FpsPlayableCharacterPolicy {
            id: "generic_entity".to_owned(),
            family: "Test".to_owned(),
            display_name: "Generic Entity".to_owned(),
            runtime_ready: true,
            runtime_model_ref: Some("models/test/generic.ydd@generic".to_owned()),
            target_height: 1.8,
            presentation: FpsCharacterPresentationPolicy {
                equipment_arm_ik: true,
                equipment_arm_ik_rig: None,
                ..FpsCharacterPresentationPolicy::default()
            },
            ..FpsPlayableCharacterPolicy::default()
        };
        character
            .validate()
            .expect("optional equipment IK must not reject a valid visual entity");
    }

    #[test]
    fn callback_export_is_not_a_ysc_selector() {
        let callbacks = FpsCallbackExports {
            interaction: "on_interaction".to_owned(),
            hit: "scripts/foo.ysc@on_hit".to_owned(),
            mission_event: "on_mission_event".to_owned(),
        };
        assert!(callbacks.validate().is_err());
    }

    #[test]
    fn callback_damage_multiplier_must_be_finite() {
        let decision = FpsPolicyDecision {
            damage_multiplier: f32::NAN,
            ..FpsPolicyDecision::default()
        };
        assert!(decision.validate().is_err());
    }

    #[test]
    fn character_menu_policy_validates_semantic_toggle_contract() {
        let mut policy = FpsCharacterMenuPolicySnapshot::default();
        policy.title = "MODEL".to_owned();
        policy.validate().expect("default semantic menu policy");

        policy.toggle_action = "KeyM".to_owned();
        policy
            .validate()
            .expect("policy accepts provider-authored semantic action ids");

        policy.toggle_action = "bad action with spaces".to_owned();
        assert!(policy.validate().is_err());
    }
}
