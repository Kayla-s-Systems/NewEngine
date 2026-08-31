use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u64);

pub const SHARED_UNARMED_WEAPON_ITEM_NAME: &str = "weapon.unarmed";

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

impl ItemId {
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = normalize_item_name(name)?;
        Some(Self(stable_hash64(normalized.as_bytes())))
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemInstanceId(pub u64);

impl ItemInstanceId {
    /// Virtual, non-inventory weapon instance representing the character's own body/hands.
    /// Inventory allocation already rejects zero, so this identity cannot alias a real item.
    pub const UNARMED: Self = Self(0);

    #[inline]
    pub const fn is_unarmed(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    #[default]
    Generic,
    Weapon,
    Ammo,
    Consumable,
    Component,
    Quest,
    Key,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EquipmentSlot {
    #[default]
    Primary,
    Secondary,
    Sidearm,
    Melee,
    Throwable,
    Gadget,
    Utility1,
    Utility2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ItemUseEffect {
    #[default]
    None,
    Heal {
        amount: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemDefinition {
    pub model_ref: Option<String>,
    pub material_library_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: [f32; 3],
    pub color: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl WorldItemDefinition {
    pub fn for_kind(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Weapon => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.42, 0.12, 0.10],
                color: [0.22, 0.27, 0.32, 1.0],
                pickup_half_extents: [0.42, 0.12, 0.10],
                respawn_seconds: 0.0,
            },
            ItemKind::Ammo => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.18, 0.10, 0.14],
                color: [0.72, 0.52, 0.18, 1.0],
                pickup_half_extents: [0.18, 0.10, 0.14],
                respawn_seconds: 0.0,
            },
            ItemKind::Consumable => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.20, 0.14, 0.22],
                color: [0.74, 0.18, 0.22, 1.0],
                pickup_half_extents: [0.20, 0.14, 0.22],
                respawn_seconds: 0.0,
            },
            ItemKind::Key | ItemKind::Quest => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_TORUS,
                scale: [0.16, 0.16, 0.05],
                color: [0.25, 0.70, 0.92, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.08],
                respawn_seconds: 0.0,
            },
            ItemKind::Generic | ItemKind::Component => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_SPHERE_UV,
                scale: [0.16, 0.16, 0.16],
                color: [0.48, 0.55, 0.65, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.16],
                respawn_seconds: 0.0,
            },
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.scale = sanitize_positive_vec3(self.scale, 0.01, 20.0);
        self.pickup_half_extents = sanitize_positive_vec3(self.pickup_half_extents, 0.01, 10.0);
        self.color = self.color.map(|value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                1.0
            }
        });
        self.respawn_seconds = sanitize_non_negative(self.respawn_seconds).min(86_400.0);
        self.model_ref = self
            .model_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self.material_library_ref = self
            .material_library_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self
    }
}

impl Default for WorldItemDefinition {
    fn default() -> Self {
        Self::for_kind(ItemKind::Generic)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemPresentation {
    pub visual_entity: EntityId,
    pub model_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: Vec3,
    pub color: [f32; 4],
    pub pickup_half_extents: Vec3,
    /// True only after the authored model/material hierarchy has been admitted.
    /// Authored items intentionally do not expose the generic fallback primitive while false.
    pub authored_visual_admitted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldItemVisualPart {
    pub owner: EntityId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldItemRuntime {
    pub persistent_id: u64,
    pub spawn_position: Vec3,
    pub original_quantity: u32,
    pub respawn_seconds: f32,
    pub respawn_remaining: f32,
    pub pickup_cooldown_remaining: f32,
    pub dropped: bool,
}

impl WorldItemRuntime {
    #[inline]
    pub fn persistent_source(
        persistent_id: u64,
        spawn_position: Vec3,
        quantity: u32,
        respawn_seconds: f32,
    ) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: sanitize_non_negative(respawn_seconds),
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.0,
            dropped: false,
        }
    }

    #[inline]
    pub fn dropped(persistent_id: u64, spawn_position: Vec3, quantity: u32) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: 0.0,
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.25,
            dropped: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponFireMode {
    #[default]
    SemiAuto,
    Automatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FiringPatternKind {
    #[default]
    Semi,
    Automatic,
    Burst,
    Charge,
    SpinUp,
    Pump,
    BoltAction,
    Binary,
    ScriptedSequence,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiringPatternDefinition {
    pub kind: FiringPatternKind,
    pub bursts_min: u8,
    pub bursts_max: u8,
    pub shots_per_burst_min: u8,
    pub shots_per_burst_max: u8,
    pub time_between_shots: f32,
    pub time_between_bursts: f32,
    pub delay_before_firing: f32,
    pub burst_cooldown: f32,
}

impl Default for FiringPatternDefinition {
    fn default() -> Self {
        Self::from_fire_mode(WeaponFireMode::SemiAuto, 0.1)
    }
}

impl FiringPatternDefinition {
    pub fn from_fire_mode(mode: WeaponFireMode, fire_interval: f32) -> Self {
        Self {
            kind: match mode {
                WeaponFireMode::SemiAuto => FiringPatternKind::Semi,
                WeaponFireMode::Automatic => FiringPatternKind::Automatic,
            },
            bursts_min: 1,
            bursts_max: 1,
            shots_per_burst_min: 1,
            shots_per_burst_max: 1,
            time_between_shots: fire_interval,
            time_between_bursts: 0.0,
            delay_before_firing: 0.0,
            burst_cooldown: 0.0,
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.bursts_min = self.bursts_min.clamp(1, 32);
        self.bursts_max = self.bursts_max.clamp(self.bursts_min, 32);
        self.shots_per_burst_min = self.shots_per_burst_min.clamp(1, 64);
        self.shots_per_burst_max = self.shots_per_burst_max.clamp(self.shots_per_burst_min, 64);
        self.time_between_shots = finite_or(self.time_between_shots, 0.1).clamp(0.01, 60.0);
        self.time_between_bursts = finite_or(self.time_between_bursts, 0.0).clamp(0.0, 60.0);
        self.delay_before_firing = finite_or(self.delay_before_firing, 0.0).clamp(0.0, 60.0);
        self.burst_cooldown = finite_or(self.burst_cooldown, 0.0).clamp(0.0, 60.0);
        self
    }
}

/// Coarse weapon taxonomy. Ammo is deliberately not part of weapon identity: both Unarmed and
/// Melee are weapons, but neither requires ammunition or exposes firearm ADS/fire/reload actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponType {
    #[default]
    Unarmed,
    Melee,
    Firearm,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponCapabilities {
    pub melee: bool,
    pub fire: bool,
    pub aim: bool,
    pub reload: bool,
    pub uses_ammo: bool,
}

impl WeaponType {
    #[inline]
    pub const fn capabilities(self) -> WeaponCapabilities {
        match self {
            Self::Unarmed | Self::Melee => WeaponCapabilities {
                melee: true,
                fire: false,
                aim: false,
                reload: false,
                uses_ammo: false,
            },
            Self::Firearm => WeaponCapabilities {
                melee: false,
                fire: true,
                aim: true,
                reload: true,
                uses_ammo: true,
            },
        }
    }

    /// Default selection/order rank. Rank is classification priority only; it never changes damage.
    #[inline]
    pub const fn default_rank(self) -> u16 {
        match self {
            Self::Unarmed => 0,
            Self::Melee => 100,
            Self::Firearm => 200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeleeWeaponTuning {
    pub damage: f32,
    pub range: f32,
    pub attack_interval: f32,
}

impl MeleeWeaponTuning {
    pub fn sanitized(self) -> Self {
        Self {
            damage: sanitize_non_negative(self.damage).clamp(0.0, 10_000.0),
            range: sanitize_non_negative(self.range).clamp(0.1, 8.0),
            attack_interval: sanitize_non_negative(self.attack_interval).clamp(0.05, 10.0),
        }
    }
}

impl Default for MeleeWeaponTuning {
    fn default() -> Self {
        Self {
            damage: 18.0,
            range: 1.35,
            attack_interval: 0.45,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirearmWeaponDefinition {
    pub tuning: HitscanWeaponTuning,
    pub ammo_item: ItemId,
    /// Compatibility shorthand retained for old consumers; runtime firing uses `firing_pattern`.
    pub fire_mode: WeaponFireMode,
    pub firing_pattern: FiringPatternDefinition,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponItemDefinition {
    pub weapon_type: WeaponType,
    pub rank: u16,
    pub melee: Option<MeleeWeaponTuning>,
    pub firearm: Option<FirearmWeaponDefinition>,
}

impl WeaponItemDefinition {
    #[inline]
    pub fn unarmed(rank: u16, tuning: MeleeWeaponTuning) -> Self {
        Self {
            weapon_type: WeaponType::Unarmed,
            rank,
            melee: Some(tuning.sanitized()),
            firearm: None,
        }
    }

    #[inline]
    pub fn melee(rank: u16, tuning: MeleeWeaponTuning) -> Self {
        Self {
            weapon_type: WeaponType::Melee,
            rank,
            melee: Some(tuning.sanitized()),
            firearm: None,
        }
    }

    #[inline]
    pub fn firearm(
        rank: u16,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
    ) -> Self {
        Self {
            weapon_type: WeaponType::Firearm,
            rank,
            melee: None,
            firearm: Some(FirearmWeaponDefinition {
                tuning: tuning.sanitized(),
                ammo_item,
                fire_mode,
                firing_pattern: FiringPatternDefinition::from_fire_mode(fire_mode, tuning.fire_interval),
            }),
        }
    }

    #[inline]
    pub fn firearm_with_pattern(
        rank: u16,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
        firing_pattern: FiringPatternDefinition,
    ) -> Self {
        let mut weapon = Self::firearm(rank, tuning, ammo_item, fire_mode);
        if let Some(firearm) = weapon.firearm.as_mut() {
            firearm.firing_pattern = firing_pattern.sanitized();
        }
        weapon
    }

    #[inline]
    pub const fn capabilities(self) -> WeaponCapabilities {
        self.weapon_type.capabilities()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponAudioAction {
    #[default]
    Fire,
    ReloadStart,
    ReloadComplete,
    Equip,
    Unequip,
    Empty,
    ShellEject,
    ShellContactSmall,
    ShellContactMedium,
    ShellContactHard,
    ShellContactSoft,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponAnimationDefinition {
    pub skeleton: Option<String>,
    pub animation_dictionary: Option<String>,
    pub idle: Option<String>,
    pub fire: Option<String>,
    pub reload: Option<String>,
    pub spawn_pose: Option<String>,
}

impl WeaponAnimationDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.skeleton = clean(self.skeleton);
        self.animation_dictionary = clean(self.animation_dictionary);
        self.idle = clean(self.idle);
        self.fire = clean(self.fire);
        self.reload = clean(self.reload);
        self.spawn_pose = clean(self.spawn_pose);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponAudioDefinition {
    pub fire: Option<String>,
    pub reload_start: Option<String>,
    pub reload_complete: Option<String>,
    pub equip: Option<String>,
    pub unequip: Option<String>,
    pub empty: Option<String>,
    pub shell_eject: Option<String>,
    pub shell_contact_small: Option<String>,
    pub shell_contact_medium: Option<String>,
    pub shell_contact_hard: Option<String>,
    pub shell_contact_soft: Option<String>,
}

impl WeaponAudioDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.fire = clean(self.fire);
        self.reload_start = clean(self.reload_start);
        self.reload_complete = clean(self.reload_complete);
        self.equip = clean(self.equip);
        self.unequip = clean(self.unequip);
        self.empty = clean(self.empty);
        self.shell_eject = clean(self.shell_eject);
        self.shell_contact_small = clean(self.shell_contact_small);
        self.shell_contact_medium = clean(self.shell_contact_medium);
        self.shell_contact_hard = clean(self.shell_contact_hard);
        self.shell_contact_soft = clean(self.shell_contact_soft);
        self
    }

    #[inline]
    pub fn clip(&self, action: WeaponAudioAction) -> Option<&str> {
        match action {
            WeaponAudioAction::Fire => self.fire.as_deref(),
            WeaponAudioAction::ReloadStart => self.reload_start.as_deref(),
            WeaponAudioAction::ReloadComplete => self.reload_complete.as_deref(),
            WeaponAudioAction::Equip => self.equip.as_deref(),
            WeaponAudioAction::Unequip => self.unequip.as_deref(),
            WeaponAudioAction::Empty => self.empty.as_deref(),
            WeaponAudioAction::ShellEject => self.shell_eject.as_deref(),
            WeaponAudioAction::ShellContactSmall => self.shell_contact_small.as_deref(),
            WeaponAudioAction::ShellContactMedium => self.shell_contact_medium.as_deref(),
            WeaponAudioAction::ShellContactHard => self.shell_contact_hard.as_deref(),
            WeaponAudioAction::ShellContactSoft => self.shell_contact_soft.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponPresentationDefinition {
    pub enabled: bool,
    pub handle_from_root: [f32; 3],
    pub muzzle_from_root: [f32; 3],
    pub left_grip_from_handle: [f32; 3],
    pub stock_contact_from_handle: [f32; 3],
    pub ready_shoulder_pocket_offset: [f32; 3],
    pub ads_shoulder_pocket_offset: [f32; 3],
    pub fire_kick_duration_seconds: f32,
    pub fire_kick_pitch_radians: f32,
    /// Third-person ReadyHold body -> weapon native-rig rotation. The runtime must compose
    /// `native_rig_to_runtime_basis` after this quaternion exactly once.
    pub ready_body_to_root_rotation: [f32; 4],
    pub ready_right_elbow_pole_offset: [f32; 3],
    pub ready_left_elbow_pole_offset: [f32; 3],
    pub ready_left_palm_to_left_grip: [f32; 3],
    pub ready_right_palm_to_weapon: [f32; 4],
    pub ready_left_palm_to_weapon: [f32; 4],
    pub right_palm_to_handle: [f32; 3],
    pub right_palm_to_native_rig: [f32; 4],
    /// The single weapon native-rig -> North Star canonical runtime basis correction.
    /// Third-person, first-person, grip, muzzle and ADS presentation must all consume this same
    /// basis; view-specific orientation compensation is forbidden.
    pub native_rig_to_runtime_basis: [f32; 4],
    /// Camera/viewmodel hip handle placement. Kept separate from anatomical full-body reach.
    pub first_person_hip_handle_offset: [f32; 3],
    /// Camera-owned full-body FPP handle placement. This must keep both authored arm contacts
    /// physically reachable; weapon definitions own the value rather than a runtime reach clamp.
    pub first_person_full_body_hip_handle_offset: [f32; 3],
    pub ads_rear_sight_from_handle: [f32; 3],
    pub ads_front_sight_from_handle: [f32; 3],
    pub ads_camera_to_rear_sight: [f32; 3],
    pub first_person_hip_convergence_m: f32,
    /// Response speed for authored ADS/ready interpolation.
    pub aim_response_hz: f32,
    /// Maximum bounded secondary angular lag in hip/ready presentation.
    pub secondary_hip_max_angle_radians: f32,
    /// Maximum bounded secondary angular lag while aiming.
    pub secondary_ads_max_angle_radians: f32,
    /// Angular target-motion inertia gain.
    pub secondary_angular_inertia_gain: f32,
    /// Player acceleration -> weapon inertia gain.
    pub secondary_movement_inertia_gain: f32,
    pub secondary_natural_hz_hip: f32,
    pub secondary_natural_hz_ads: f32,
    pub secondary_obstruction_hz_boost: f32,
}

impl Default for WeaponPresentationDefinition {
    fn default() -> Self {
        Self {
            enabled: false,
            handle_from_root: [0.0; 3],
            muzzle_from_root: [0.0, 0.0, 0.5],
            left_grip_from_handle: [0.0, 0.0, 0.25],
            stock_contact_from_handle: [0.0, 0.0, -0.25],
            ready_shoulder_pocket_offset: [0.0, -0.1, -0.04],
            ads_shoulder_pocket_offset: [0.0, -0.08, -0.03],
            fire_kick_duration_seconds: 0.15,
            fire_kick_pitch_radians: 0.0,
            ready_body_to_root_rotation: [0.0, 0.0, 0.0, 1.0],
            ready_right_elbow_pole_offset: [-0.15, -0.14, 0.06],
            ready_left_elbow_pole_offset: [0.15, -0.16, 0.08],
            ready_left_palm_to_left_grip: [0.0; 3],
            ready_right_palm_to_weapon: [0.0, 0.0, 0.0, 1.0],
            ready_left_palm_to_weapon: [0.0, 0.0, 0.0, 1.0],
            right_palm_to_handle: [0.0; 3],
            right_palm_to_native_rig: [0.0, 0.0, 0.0, 1.0],
            native_rig_to_runtime_basis: [0.0, 0.0, 0.0, 1.0],
            first_person_hip_handle_offset: [0.2, -0.2, -0.5],
            // Compatibility default only. Authored item compilation inherits the ordinary FPP
            // offset when no explicit full-body value exists.
            first_person_full_body_hip_handle_offset: [0.2, -0.2, -0.5],
            ads_rear_sight_from_handle: [0.0; 3],
            ads_front_sight_from_handle: [0.0, 0.0, 0.4],
            ads_camera_to_rear_sight: [0.0, 0.0, -0.075],
            first_person_hip_convergence_m: 12.0,
            aim_response_hz: 18.0,
            secondary_hip_max_angle_radians: 5.0_f32.to_radians(),
            secondary_ads_max_angle_radians: 2.25_f32.to_radians(),
            secondary_angular_inertia_gain: 0.38,
            secondary_movement_inertia_gain: 1.0,
            secondary_natural_hz_hip: 5.4,
            secondary_natural_hz_ads: 9.0,
            secondary_obstruction_hz_boost: 6.0,
        }
    }
}

impl WeaponPresentationDefinition {
    pub fn sanitized(mut self) -> Self {
        fn vec3(value: [f32; 3], fallback: [f32; 3], limit: f32) -> [f32; 3] {
            let mut out = value;
            for (index, component) in out.iter_mut().enumerate() {
                if !component.is_finite() || component.abs() > limit {
                    *component = fallback[index];
                }
            }
            out
        }
        fn quat(value: [f32; 4]) -> [f32; 4] {
            let len2 = value.iter().map(|value| value * value).sum::<f32>();
            if value.iter().all(|value| value.is_finite()) && len2 > 1.0e-8 {
                let inv = len2.sqrt().recip();
                [
                    value[0] * inv,
                    value[1] * inv,
                    value[2] * inv,
                    value[3] * inv,
                ]
            } else {
                [0.0, 0.0, 0.0, 1.0]
            }
        }
        let fallback = Self::default();
        self.handle_from_root = vec3(self.handle_from_root, fallback.handle_from_root, 10.0);
        self.muzzle_from_root = vec3(self.muzzle_from_root, fallback.muzzle_from_root, 10.0);
        self.left_grip_from_handle = vec3(
            self.left_grip_from_handle,
            fallback.left_grip_from_handle,
            10.0,
        );
        self.stock_contact_from_handle = vec3(
            self.stock_contact_from_handle,
            fallback.stock_contact_from_handle,
            10.0,
        );
        self.ready_shoulder_pocket_offset = vec3(
            self.ready_shoulder_pocket_offset,
            fallback.ready_shoulder_pocket_offset,
            5.0,
        );
        self.ads_shoulder_pocket_offset = vec3(
            self.ads_shoulder_pocket_offset,
            fallback.ads_shoulder_pocket_offset,
            5.0,
        );
        self.ready_right_elbow_pole_offset = vec3(
            self.ready_right_elbow_pole_offset,
            fallback.ready_right_elbow_pole_offset,
            5.0,
        );
        self.ready_left_elbow_pole_offset = vec3(
            self.ready_left_elbow_pole_offset,
            fallback.ready_left_elbow_pole_offset,
            5.0,
        );
        self.ready_left_palm_to_left_grip = vec3(
            self.ready_left_palm_to_left_grip,
            fallback.ready_left_palm_to_left_grip,
            5.0,
        );
        self.right_palm_to_handle = vec3(
            self.right_palm_to_handle,
            fallback.right_palm_to_handle,
            5.0,
        );
        self.first_person_hip_handle_offset = vec3(
            self.first_person_hip_handle_offset,
            fallback.first_person_hip_handle_offset,
            5.0,
        );
        self.first_person_full_body_hip_handle_offset = vec3(
            self.first_person_full_body_hip_handle_offset,
            self.first_person_hip_handle_offset,
            5.0,
        );
        self.ads_rear_sight_from_handle = vec3(
            self.ads_rear_sight_from_handle,
            fallback.ads_rear_sight_from_handle,
            5.0,
        );
        self.ads_front_sight_from_handle = vec3(
            self.ads_front_sight_from_handle,
            fallback.ads_front_sight_from_handle,
            5.0,
        );
        self.ads_camera_to_rear_sight = vec3(
            self.ads_camera_to_rear_sight,
            fallback.ads_camera_to_rear_sight,
            5.0,
        );
        self.ready_body_to_root_rotation = quat(self.ready_body_to_root_rotation);
        self.ready_right_palm_to_weapon = quat(self.ready_right_palm_to_weapon);
        self.ready_left_palm_to_weapon = quat(self.ready_left_palm_to_weapon);
        self.right_palm_to_native_rig = quat(self.right_palm_to_native_rig);
        self.native_rig_to_runtime_basis = quat(self.native_rig_to_runtime_basis);
        self.fire_kick_duration_seconds = if self.fire_kick_duration_seconds.is_finite() {
            self.fire_kick_duration_seconds.clamp(0.001, 10.0)
        } else {
            fallback.fire_kick_duration_seconds
        };
        self.fire_kick_pitch_radians = if self.fire_kick_pitch_radians.is_finite() {
            self.fire_kick_pitch_radians
                .clamp(-std::f32::consts::PI, std::f32::consts::PI)
        } else {
            0.0
        };
        self.first_person_hip_convergence_m = if self.first_person_hip_convergence_m.is_finite() {
            self.first_person_hip_convergence_m.clamp(0.1, 10_000.0)
        } else {
            fallback.first_person_hip_convergence_m
        };
        self.aim_response_hz =
            finite_or(self.aim_response_hz, fallback.aim_response_hz).clamp(0.1, 120.0);
        self.secondary_hip_max_angle_radians = finite_or(
            self.secondary_hip_max_angle_radians,
            fallback.secondary_hip_max_angle_radians,
        )
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
        self.secondary_ads_max_angle_radians = finite_or(
            self.secondary_ads_max_angle_radians,
            fallback.secondary_ads_max_angle_radians,
        )
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
        self.secondary_angular_inertia_gain = finite_or(
            self.secondary_angular_inertia_gain,
            fallback.secondary_angular_inertia_gain,
        )
        .clamp(0.0, 4.0);
        self.secondary_movement_inertia_gain = finite_or(
            self.secondary_movement_inertia_gain,
            fallback.secondary_movement_inertia_gain,
        )
        .clamp(0.0, 4.0);
        self.secondary_natural_hz_hip = finite_or(
            self.secondary_natural_hz_hip,
            fallback.secondary_natural_hz_hip,
        )
        .clamp(0.1, 120.0);
        self.secondary_natural_hz_ads = finite_or(
            self.secondary_natural_hz_ads,
            fallback.secondary_natural_hz_ads,
        )
        .clamp(0.1, 120.0);
        self.secondary_obstruction_hz_boost = finite_or(
            self.secondary_obstruction_hz_boost,
            fallback.secondary_obstruction_hz_boost,
        )
        .clamp(0.0, 120.0);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponVfxDefinition {
    pub shot: Option<String>,
    /// Independent single-frame/swept tracer presentation for an already-resolved shot segment.
    pub tracer: Option<String>,
    /// Shallow-angle collision sweetener spawned only when ballistics schedules a ricochet trace.
    pub ricochet: Option<String>,
    pub impact_default: Option<String>,
    pub impact_by_surface: BTreeMap<String, String>,
}

impl WeaponVfxDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.shot = clean(self.shot);
        self.tracer = clean(self.tracer);
        self.ricochet = clean(self.ricochet);
        self.impact_default = clean(self.impact_default);
        self.impact_by_surface = self
            .impact_by_surface
            .into_iter()
            .filter_map(|(surface, effect)| {
                let surface = surface.trim().to_ascii_lowercase();
                let effect = effect.trim().replace('\\', "/");
                (!surface.is_empty() && !effect.is_empty()).then_some((surface, effect))
            })
            .collect();
        self
    }

    #[inline]
    pub fn impact_effect(&self, surface: Option<&str>) -> Option<&str> {
        let surface = surface.map(|value| value.trim().to_ascii_lowercase());
        if let Some(surface) = surface.as_deref() {
            if let Some(exact) = self.impact_by_surface.get(surface) {
                return Some(exact.as_str());
            }
            // Physics surfaces are commonly hierarchical (`surface.metal.floor`,
            // `environment.concrete.wall`, ...). Project-authored impact rules are semantic
            // match tokens; prefer the longest matching token so a specific rule wins over a
            // broad material family without requiring runtime hard-coding.
            if let Some((_, effect)) = self
                .impact_by_surface
                .iter()
                .filter(|(needle, _)| !needle.is_empty() && surface.contains(needle.as_str()))
                .max_by_key(|(needle, _)| needle.len())
            {
                return Some(effect.as_str());
            }
        }
        self.impact_default.as_deref()
    }

    pub fn effect_refs(&self) -> impl Iterator<Item = &str> {
        self.shot
            .iter()
            .map(String::as_str)
            .chain(self.impact_default.iter().map(String::as_str))
            .chain(self.impact_by_surface.values().map(String::as_str))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponCasingDefinition {
    /// Runtime model dictionary; `variants` are entry selectors inside this dictionary.
    pub model_dictionary: Option<String>,
    pub variants: Vec<String>,
    pub material_ref: Option<String>,
    /// Dynamic rigid-body box half extents in metres.
    pub half_extents: [f32; 3],
    /// Delay from the shot event to physical casing spawn.
    pub ejection_delay_seconds: f32,
    /// Optional joint/socket on the weapon skeleton. If authored, casing emission follows the
    /// animated weapon entity rather than reconstructing a pose from the player or camera.
    pub ejection_joint: Option<String>,
    /// Fraction of measured socket linear/angular velocity inherited by the ejected casing.
    pub inherit_socket_linear_velocity: f32,
    pub inherit_socket_angular_velocity: f32,
    /// Local basis coefficients `[right, up, forward]` relative to the ejection socket pose.
    pub origin_local: [f32; 3],
    pub velocity_local: [f32; 3],
    /// Signed scalar jitter is multiplied component-wise by this vector.
    pub velocity_jitter: [f32; 3],
    /// Local axis used to orient the casing model; interpreted in `[right, up, forward]`.
    pub axis_local: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub angular_velocity_jitter: [f32; 3],
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub contact_min_impulse: f32,
    pub contact_medium_impulse: f32,
    pub contact_hard_impulse: f32,
    /// Case-insensitive surface-id substrings that select the soft-contact cue.
    pub soft_surface_contains: Vec<String>,
}

impl Default for WeaponCasingDefinition {
    fn default() -> Self {
        // Disabled schema default. A concrete weapon must author its casing contract.
        Self {
            model_dictionary: None,
            variants: Vec::new(),
            material_ref: None,
            half_extents: [0.01, 0.01, 0.01],
            ejection_delay_seconds: 0.0,
            ejection_joint: None,
            inherit_socket_linear_velocity: 1.0,
            inherit_socket_angular_velocity: 0.35,
            origin_local: [0.0; 3],
            velocity_local: [0.0; 3],
            velocity_jitter: [0.0; 3],
            axis_local: [1.0, 0.0, 0.0],
            angular_velocity: [0.0; 3],
            angular_velocity_jitter: [0.0; 3],
            friction: 0.4,
            restitution: 0.1,
            density: 1.0,
            contact_min_impulse: 0.0,
            contact_medium_impulse: 0.0,
            contact_hard_impulse: 0.0,
            soft_surface_contains: Vec::new(),
        }
    }
}

impl WeaponCasingDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        fn finite_vec3(mut value: [f32; 3], fallback: [f32; 3], limit: f32) -> [f32; 3] {
            for index in 0..3 {
                value[index] = if value[index].is_finite() {
                    value[index].clamp(-limit, limit)
                } else {
                    fallback[index]
                };
            }
            value
        }
        self.model_dictionary = clean(self.model_dictionary);
        self.material_ref = clean(self.material_ref);
        self.ejection_joint = clean(self.ejection_joint);
        self.variants = self
            .variants
            .into_iter()
            .map(|value| value.trim().trim_start_matches('@').to_owned())
            .filter(|value| !value.is_empty() && !value.contains('/') && !value.contains('\\'))
            .collect();
        self.variants.sort();
        self.variants.dedup();
        self.half_extents = sanitize_positive_vec3(self.half_extents, 0.0005, 1.0);
        self.ejection_delay_seconds = if self.ejection_delay_seconds.is_finite() {
            self.ejection_delay_seconds.clamp(0.0, 2.0)
        } else {
            0.0
        };
        self.inherit_socket_linear_velocity =
            finite_or(self.inherit_socket_linear_velocity, 1.0).clamp(0.0, 4.0);
        self.inherit_socket_angular_velocity =
            finite_or(self.inherit_socket_angular_velocity, 0.35).clamp(0.0, 4.0);
        self.origin_local = finite_vec3(self.origin_local, [0.0; 3], 10.0);
        self.velocity_local = finite_vec3(self.velocity_local, [0.0; 3], 100.0);
        self.velocity_jitter = finite_vec3(self.velocity_jitter, [0.0; 3], 100.0);
        self.axis_local = finite_vec3(self.axis_local, [1.0, 0.0, 0.0], 1.0);
        if self
            .axis_local
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            <= 1.0e-8
        {
            self.axis_local = [1.0, 0.0, 0.0];
        }
        self.angular_velocity = finite_vec3(self.angular_velocity, [0.0; 3], 500.0);
        self.angular_velocity_jitter = finite_vec3(self.angular_velocity_jitter, [0.0; 3], 500.0);
        self.friction = if self.friction.is_finite() {
            self.friction.clamp(0.0, 2.0)
        } else {
            0.4
        };
        self.restitution = if self.restitution.is_finite() {
            self.restitution.clamp(0.0, 1.0)
        } else {
            0.1
        };
        self.density = if self.density.is_finite() {
            // Permit physically meaningful authored material densities (e.g. brass/steel) while
            // keeping pathological values bounded for backend stability.
            self.density.clamp(0.01, 25_000.0)
        } else {
            1.0
        };
        self.contact_min_impulse = finite_or(self.contact_min_impulse, 0.0).max(0.0);
        self.contact_medium_impulse =
            finite_or(self.contact_medium_impulse, self.contact_min_impulse)
                .max(self.contact_min_impulse);
        self.contact_hard_impulse =
            finite_or(self.contact_hard_impulse, self.contact_medium_impulse)
                .max(self.contact_medium_impulse);
        self.soft_surface_contains = self
            .soft_surface_contains
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.soft_surface_contains.sort();
        self.soft_surface_contains.dedup();
        self
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.model_dictionary.is_some() && !self.variants.is_empty()
    }

    pub fn model_ref(&self, variant_index: usize) -> Option<String> {
        let dictionary = self.model_dictionary.as_deref()?;
        let selector = self
            .variants
            .get(variant_index % self.variants.len().max(1))?;
        Some(format!("{dictionary}@{selector}"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmmoProjectileType {
    #[default]
    Instant,
    Physical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AmmoDefinition {
    pub caliber: String,
    pub projectile_type: AmmoProjectileType,
    pub projectile_mass_kg: f32,
    pub muzzle_velocity_mps: f32,
    /// Initial penetration budget in joules. Material traversal consumes this budget.
    pub penetration_energy_j: f32,
    pub max_penetration_m: f32,
    pub drag_coefficient: f32,
    pub damage_multiplier: f32,
    pub impulse_multiplier: f32,
    pub falloff_start_m: f32,
    pub falloff_end_m: f32,
    pub falloff_min_multiplier: f32,
    pub tracer: bool,
    pub impact_profile: Option<String>,
}

impl Default for AmmoDefinition {
    fn default() -> Self {
        Self {
            caliber: "generic".to_owned(),
            projectile_type: AmmoProjectileType::Instant,
            projectile_mass_kg: 0.008,
            muzzle_velocity_mps: 350.0,
            penetration_energy_j: 400.0,
            max_penetration_m: 0.35,
            drag_coefficient: 0.0,
            damage_multiplier: 1.0,
            impulse_multiplier: 1.0,
            falloff_start_m: 0.0,
            falloff_end_m: 100.0,
            falloff_min_multiplier: 1.0,
            tracer: false,
            impact_profile: None,
        }
    }
}

impl AmmoDefinition {
    pub fn sanitized(mut self) -> Self {
        self.caliber = self.caliber.trim().to_ascii_lowercase();
        if self.caliber.is_empty() {
            self.caliber = "generic".to_owned();
        }
        self.projectile_mass_kg = finite_or(self.projectile_mass_kg, 0.008).clamp(0.0001, 1.0);
        self.muzzle_velocity_mps = finite_or(self.muzzle_velocity_mps, 350.0).clamp(1.0, 2_500.0);
        self.penetration_energy_j = finite_or(self.penetration_energy_j, 0.0).clamp(0.0, 250_000.0);
        self.max_penetration_m = finite_or(self.max_penetration_m, 0.0).clamp(0.0, 10.0);
        self.drag_coefficient = finite_or(self.drag_coefficient, 0.0).clamp(0.0, 10.0);
        self.damage_multiplier = finite_or(self.damage_multiplier, 1.0).clamp(0.0, 20.0);
        self.impulse_multiplier = finite_or(self.impulse_multiplier, 1.0).clamp(0.0, 20.0);
        self.falloff_start_m = finite_or(self.falloff_start_m, 0.0).clamp(0.0, 10_000.0);
        self.falloff_end_m = finite_or(self.falloff_end_m, self.falloff_start_m.max(1.0))
            .clamp(self.falloff_start_m.max(0.001), 10_000.0);
        self.falloff_min_multiplier = finite_or(self.falloff_min_multiplier, 1.0).clamp(0.0, 1.0);
        self.impact_profile = self
            .impact_profile
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self
    }

    #[inline]
    pub fn kinetic_energy_j(&self) -> f32 {
        0.5 * self.projectile_mass_kg * self.muzzle_velocity_mps * self.muzzle_velocity_mps
    }

    #[inline]
    pub fn momentum_ns(&self) -> f32 {
        self.projectile_mass_kg * self.muzzle_velocity_mps
    }

    pub fn falloff_multiplier_at(&self, distance_m: f32) -> f32 {
        let value = self.clone().sanitized();
        if distance_m <= value.falloff_start_m {
            return 1.0;
        }
        if distance_m >= value.falloff_end_m {
            return value.falloff_min_multiplier;
        }
        let alpha = ((distance_m - value.falloff_start_m)
            / (value.falloff_end_m - value.falloff_start_m).max(0.001))
            .clamp(0.0, 1.0);
        1.0 + (value.falloff_min_multiplier - 1.0) * alpha
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponComponentModifiers {
    pub accuracy_multiplier: f32,
    pub recoil_multiplier: f32,
    pub damage_multiplier: f32,
    pub falloff_multiplier: f32,
    pub muzzle_velocity_multiplier: f32,
    pub penetration_multiplier: f32,
    pub audio_gain_multiplier: f32,
    pub presentation_offset_local: [f32; 3],
}

impl Default for WeaponComponentModifiers {
    fn default() -> Self {
        Self {
            accuracy_multiplier: 1.0,
            recoil_multiplier: 1.0,
            damage_multiplier: 1.0,
            falloff_multiplier: 1.0,
            muzzle_velocity_multiplier: 1.0,
            penetration_multiplier: 1.0,
            audio_gain_multiplier: 1.0,
            presentation_offset_local: [0.0; 3],
        }
    }
}

impl WeaponComponentModifiers {
    pub fn sanitized(self) -> Self {
        Self {
            accuracy_multiplier: finite_or(self.accuracy_multiplier, 1.0).clamp(0.05, 20.0),
            recoil_multiplier: finite_or(self.recoil_multiplier, 1.0).clamp(0.0, 20.0),
            damage_multiplier: finite_or(self.damage_multiplier, 1.0).clamp(0.0, 20.0),
            falloff_multiplier: finite_or(self.falloff_multiplier, 1.0).clamp(0.0, 20.0),
            muzzle_velocity_multiplier: finite_or(self.muzzle_velocity_multiplier, 1.0).clamp(0.05, 20.0),
            penetration_multiplier: finite_or(self.penetration_multiplier, 1.0).clamp(0.0, 20.0),
            audio_gain_multiplier: finite_or(self.audio_gain_multiplier, 1.0).clamp(0.0, 4.0),
            presentation_offset_local: self.presentation_offset_local.map(|value| finite_or(value, 0.0).clamp(-2.0, 2.0)),
        }
    }

    pub fn combine(self, other: Self) -> Self {
        let a = self.sanitized();
        let b = other.sanitized();
        Self {
            accuracy_multiplier: a.accuracy_multiplier * b.accuracy_multiplier,
            recoil_multiplier: a.recoil_multiplier * b.recoil_multiplier,
            damage_multiplier: a.damage_multiplier * b.damage_multiplier,
            falloff_multiplier: a.falloff_multiplier * b.falloff_multiplier,
            muzzle_velocity_multiplier: a.muzzle_velocity_multiplier * b.muzzle_velocity_multiplier,
            penetration_multiplier: a.penetration_multiplier * b.penetration_multiplier,
            audio_gain_multiplier: a.audio_gain_multiplier * b.audio_gain_multiplier,
            presentation_offset_local: [
                a.presentation_offset_local[0] + b.presentation_offset_local[0],
                a.presentation_offset_local[1] + b.presentation_offset_local[1],
                a.presentation_offset_local[2] + b.presentation_offset_local[2],
            ],
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponComponentDefinition {
    pub id: String,
    pub slot: String,
    pub model_ref: Option<String>,
    pub audio_override: Option<String>,
    pub muzzle_vfx_override: Option<String>,
    pub tracer_vfx_override: Option<String>,
    pub modifiers: WeaponComponentModifiers,
}

impl WeaponComponentDefinition {
    pub fn sanitized(mut self) -> Self {
        self.id = self.id.trim().to_ascii_lowercase();
        self.slot = self.slot.trim().to_ascii_lowercase();
        let clean = |value: Option<String>| value
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self.model_ref = clean(self.model_ref);
        self.audio_override = clean(self.audio_override);
        self.muzzle_vfx_override = clean(self.muzzle_vfx_override);
        self.tracer_vfx_override = clean(self.tracer_vfx_override);
        self.modifiers = self.modifiers.sanitized();
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponComponentPointDefinition {
    pub id: String,
    pub attach_joint: String,
    pub allowed_components: Vec<String>,
}

impl WeaponComponentPointDefinition {
    pub fn sanitized(mut self) -> Self {
        self.id = self.id.trim().to_ascii_lowercase();
        self.attach_joint = self.attach_joint.trim().to_ascii_lowercase();
        self.allowed_components = self.allowed_components
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.allowed_components.sort();
        self.allowed_components.dedup();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponComponentGraphDefinition {
    pub points: Vec<WeaponComponentPointDefinition>,
    pub components: BTreeMap<String, WeaponComponentDefinition>,
    pub default_installed: BTreeMap<String, String>,
}

impl WeaponComponentGraphDefinition {
    pub fn sanitized(mut self) -> Self {
        self.points = self.points.into_iter().map(WeaponComponentPointDefinition::sanitized)
            .filter(|point| !point.id.is_empty()).collect();
        self.points.sort_by(|a, b| a.id.cmp(&b.id));
        self.points.dedup_by(|a, b| a.id == b.id);
        self.components = self.components.into_values()
            .map(WeaponComponentDefinition::sanitized)
            .filter(|component| !component.id.is_empty() && !component.slot.is_empty())
            .map(|component| (component.id.clone(), component)).collect();
        self.default_installed = self.default_installed.into_iter()
            .map(|(slot, component)| (slot.trim().to_ascii_lowercase(), component.trim().to_ascii_lowercase()))
            .filter(|(slot, component)| !slot.is_empty() && !component.is_empty())
            .collect();
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        let graph = self.clone().sanitized();
        for (slot, component_id) in &graph.default_installed {
            let point = graph.points.iter().find(|point| &point.id == slot)
                .ok_or_else(|| format!("component default references unknown slot '{slot}'"))?;
            let component = graph.components.get(component_id)
                .ok_or_else(|| format!("component default references unknown component '{component_id}'"))?;
            if component.slot != *slot || (!point.allowed_components.is_empty() && !point.allowed_components.contains(component_id)) {
                return Err(format!("component '{component_id}' is not allowed in slot '{slot}'"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponComponentInstance {
    pub component_id: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemId,
    pub name: String,
    pub definition_ref: Option<String>,
    pub display_name: String,
    pub description: String,
    pub icon_ref: Option<String>,
    pub tags: Vec<String>,
    pub kind: ItemKind,
    pub max_stack: u32,
    pub unit_weight: f32,
    pub equipment_slot: Option<EquipmentSlot>,
    pub weapon: Option<WeaponItemDefinition>,
    /// Present only for `ItemKind::Ammo`; firearm mechanics reference ammo by item identity.
    pub ammo_profile: Option<AmmoDefinition>,
    pub weapon_components: WeaponComponentGraphDefinition,
    pub weapon_presentation: WeaponPresentationDefinition,
    pub weapon_animation: WeaponAnimationDefinition,
    pub weapon_audio: WeaponAudioDefinition,
    pub weapon_vfx: WeaponVfxDefinition,
    pub weapon_casing: WeaponCasingDefinition,
    pub use_effect: ItemUseEffect,
    pub world: WorldItemDefinition,
}

impl ItemDefinition {
    pub fn stackable(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        kind: ItemKind,
        max_stack: u32,
        unit_weight: f32,
    ) -> Result<Self, String> {
        let name = normalize_item_name(name.as_ref())
            .ok_or_else(|| "item name must contain at least one valid character".to_owned())?;
        let id = ItemId(stable_hash64(name.as_bytes()));
        Ok(Self {
            id,
            name,
            definition_ref: None,
            display_name: display_name.into(),
            description: String::new(),
            icon_ref: None,
            tags: Vec::new(),
            kind,
            max_stack: max_stack.clamp(1, 1_000_000),
            unit_weight: sanitize_non_negative(unit_weight),
            equipment_slot: None,
            weapon: None,
            ammo_profile: None,
            weapon_components: WeaponComponentGraphDefinition::default(),
            weapon_presentation: WeaponPresentationDefinition::default(),
            weapon_animation: WeaponAnimationDefinition::default(),
            weapon_audio: WeaponAudioDefinition::default(),
            weapon_vfx: WeaponVfxDefinition::default(),
            weapon_casing: WeaponCasingDefinition::default(),
            use_effect: ItemUseEffect::None,
            world: WorldItemDefinition::for_kind(kind),
        })
    }

    pub fn typed_weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: Option<EquipmentSlot>,
        weapon: WeaponItemDefinition,
        unit_weight: f32,
    ) -> Result<Self, String> {
        let mut item = Self::stackable(name, display_name, ItemKind::Weapon, 1, unit_weight)?;
        item.equipment_slot = slot;
        item.weapon = Some(weapon);
        Ok(item)
    }

    /// Backward-compatible firearm constructor. Concrete weapons remain project-authored; this
    /// helper only constructs the engine's Firearm weapon type.
    pub fn weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: EquipmentSlot,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
        unit_weight: f32,
    ) -> Result<Self, String> {
        Self::typed_weapon(
            name,
            display_name,
            Some(slot),
            WeaponItemDefinition::firearm(
                WeaponType::Firearm.default_rank(),
                tuning,
                ammo_item,
                fire_mode,
            ),
            unit_weight,
        )
    }

    pub fn melee_weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: EquipmentSlot,
        rank: u16,
        tuning: MeleeWeaponTuning,
        unit_weight: f32,
    ) -> Result<Self, String> {
        Self::typed_weapon(
            name,
            display_name,
            Some(slot),
            WeaponItemDefinition::melee(rank, tuning),
            unit_weight,
        )
    }

    #[inline]
    pub fn with_ammo_profile(mut self, ammo: AmmoDefinition) -> Self {
        self.ammo_profile = Some(ammo.sanitized());
        self
    }

    pub fn with_weapon_components(mut self, graph: WeaponComponentGraphDefinition) -> Result<Self, String> {
        let graph = graph.sanitized();
        graph.validate()?;
        self.weapon_components = graph;
        Ok(self)
    }

    #[inline]
    pub fn with_weapon_presentation(mut self, presentation: WeaponPresentationDefinition) -> Self {
        self.weapon_presentation = presentation.sanitized();
        self
    }

    pub fn with_weapon_animation(mut self, animation: WeaponAnimationDefinition) -> Self {
        self.weapon_animation = animation.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_audio(mut self, audio: WeaponAudioDefinition) -> Self {
        self.weapon_audio = audio.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_vfx(mut self, vfx: WeaponVfxDefinition) -> Self {
        self.weapon_vfx = vfx.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_casing(mut self, casing: WeaponCasingDefinition) -> Self {
        self.weapon_casing = casing.sanitized();
        self
    }

    #[inline]
    pub fn with_definition_ref(mut self, definition_ref: impl Into<String>) -> Self {
        let value = definition_ref.into().trim().replace('\\', "/");
        self.definition_ref = (!value.is_empty()).then_some(value);
        self
    }

    #[inline]
    pub fn with_world_definition(mut self, world: WorldItemDefinition) -> Self {
        self.world = world.sanitized();
        self
    }

    #[inline]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    #[inline]
    pub fn with_icon(mut self, icon_ref: impl Into<String>) -> Self {
        let icon_ref = icon_ref.into();
        self.icon_ref = (!icon_ref.trim().is_empty()).then_some(icon_ref);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags
            .into_iter()
            .map(Into::into)
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        self.tags.sort();
        self.tags.dedup();
        self
    }

    pub fn consumable(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        max_stack: u32,
        unit_weight: f32,
        effect: ItemUseEffect,
    ) -> Result<Self, String> {
        let mut item = Self::stackable(
            name,
            display_name,
            ItemKind::Consumable,
            max_stack,
            unit_weight,
        )?;
        item.use_effect = match effect {
            ItemUseEffect::Heal { amount } => ItemUseEffect::Heal {
                amount: sanitize_non_negative(amount),
            },
            ItemUseEffect::None => ItemUseEffect::None,
        };
        Ok(item)
    }
}
