use std::collections::BTreeMap;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemPackage {
    pub schema: String,
    pub version: u32,
    pub items: Vec<AuthoredItemDefinition>,
    pub loadouts: Vec<AuthoredLoadoutDefinition>,
}

impl Default for AuthoredItemPackage {
    fn default() -> Self {
        Self {
            schema: AUTHORED_ITEM_PACKAGE_SCHEMA.to_owned(),
            version: AUTHORED_ITEM_PACKAGE_VERSION,
            items: Vec::new(),
            loadouts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemDefinition {
    pub id: String,
    pub definition_ref: String,
    pub display_name: String,
    pub description: String,
    pub icon: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub max_stack: u32,
    pub unit_weight: f32,
    pub equipment_slot: String,
    pub weapon: Option<AuthoredWeaponDefinition>,
    pub ammo_profile: Option<AuthoredAmmoDefinition>,
    pub weapon_components: Option<AuthoredWeaponComponentGraphDefinition>,
    pub weapon_animation: Option<AuthoredWeaponAnimationDefinition>,
    pub weapon_audio: Option<AuthoredWeaponAudioDefinition>,
    pub weapon_vfx: Option<AuthoredWeaponVfxDefinition>,
    pub weapon_presentation: Option<AuthoredWeaponPresentationDefinition>,
    pub weapon_casing: Option<AuthoredWeaponCasingDefinition>,
    pub use_effect: Option<AuthoredUseEffect>,
    pub world: Option<AuthoredWorldItemDefinition>,
}

impl Default for AuthoredItemDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            definition_ref: String::new(),
            display_name: String::new(),
            description: String::new(),
            icon: String::new(),
            tags: Vec::new(),
            kind: "generic".to_owned(),
            max_stack: 1,
            unit_weight: 0.0,
            equipment_slot: String::new(),
            weapon: None,
            ammo_profile: None,
            weapon_components: None,
            weapon_animation: None,
            weapon_audio: None,
            weapon_vfx: None,
            weapon_presentation: None,
            weapon_casing: None,
            use_effect: None,
            world: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredAmmoDefinition {
    pub caliber: String,
    pub projectile_type: String,
    pub projectile_mass_kg: f32,
    pub muzzle_velocity_mps: f32,
    pub penetration_energy_j: f32,
    pub max_penetration_m: f32,
    pub drag_coefficient: f32,
    pub damage_multiplier: f32,
    pub impulse_multiplier: f32,
    pub falloff_start_m: f32,
    pub falloff_end_m: f32,
    pub falloff_min_multiplier: f32,
    pub tracer: bool,
    pub impact_profile: String,
}

impl Default for AuthoredAmmoDefinition {
    fn default() -> Self {
        let runtime = AmmoDefinition::default();
        Self {
            caliber: runtime.caliber,
            projectile_type: "instant".to_owned(),
            projectile_mass_kg: runtime.projectile_mass_kg,
            muzzle_velocity_mps: runtime.muzzle_velocity_mps,
            penetration_energy_j: runtime.penetration_energy_j,
            max_penetration_m: runtime.max_penetration_m,
            drag_coefficient: runtime.drag_coefficient,
            damage_multiplier: runtime.damage_multiplier,
            impulse_multiplier: runtime.impulse_multiplier,
            falloff_start_m: runtime.falloff_start_m,
            falloff_end_m: runtime.falloff_end_m,
            falloff_min_multiplier: runtime.falloff_min_multiplier,
            tracer: runtime.tracer,
            impact_profile: String::new(),
        }
    }
}

impl AuthoredAmmoDefinition {
    pub(super) fn compile(&self) -> Result<AmmoDefinition, String> {
        let projectile_type = match self.projectile_type.trim().to_ascii_lowercase().as_str() {
            "" | "instant" | "hitscan" | "ballistic_ray" => AmmoProjectileType::Instant,
            "physical" | "projectile" => AmmoProjectileType::Physical,
            other => return Err(format!("unsupported ammo projectile_type '{other}'")),
        };
        Ok(AmmoDefinition {
            caliber: self.caliber.clone(),
            projectile_type,
            projectile_mass_kg: self.projectile_mass_kg,
            muzzle_velocity_mps: self.muzzle_velocity_mps,
            penetration_energy_j: self.penetration_energy_j,
            max_penetration_m: self.max_penetration_m,
            drag_coefficient: self.drag_coefficient,
            damage_multiplier: self.damage_multiplier,
            impulse_multiplier: self.impulse_multiplier,
            falloff_start_m: self.falloff_start_m,
            falloff_end_m: self.falloff_end_m,
            falloff_min_multiplier: self.falloff_min_multiplier,
            tracer: self.tracer,
            impact_profile: (!self.impact_profile.trim().is_empty())
                .then(|| self.impact_profile.trim().to_owned()),
        }
        .sanitized())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentModifiers {
    pub accuracy_multiplier: f32,
    pub recoil_multiplier: f32,
    pub damage_multiplier: f32,
    pub falloff_multiplier: f32,
    pub muzzle_velocity_multiplier: f32,
    pub penetration_multiplier: f32,
    pub audio_gain_multiplier: f32,
    pub presentation_offset_local: [f32; 3],
}

impl Default for AuthoredWeaponComponentModifiers {
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

impl AuthoredWeaponComponentModifiers {
    fn compile(&self) -> WeaponComponentModifiers {
        WeaponComponentModifiers {
            accuracy_multiplier: self.accuracy_multiplier,
            recoil_multiplier: self.recoil_multiplier,
            damage_multiplier: self.damage_multiplier,
            falloff_multiplier: self.falloff_multiplier,
            muzzle_velocity_multiplier: self.muzzle_velocity_multiplier,
            penetration_multiplier: self.penetration_multiplier,
            audio_gain_multiplier: self.audio_gain_multiplier,
            presentation_offset_local: self.presentation_offset_local,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentDefinition {
    pub id: String,
    pub slot: String,
    pub model_ref: String,
    pub audio_override: String,
    pub muzzle_vfx_override: String,
    pub tracer_vfx_override: String,
    pub stat_modifiers: Vec<AuthoredWeaponStatModifier>,
    pub modifiers: AuthoredWeaponComponentModifiers,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentPointDefinition {
    pub id: String,
    pub attach_joint: String,
    pub allowed_components: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentGraphDefinition {
    pub points: Vec<AuthoredWeaponComponentPointDefinition>,
    pub components: Vec<AuthoredWeaponComponentDefinition>,
    pub default_installed: BTreeMap<String, String>,
}

impl AuthoredWeaponComponentGraphDefinition {
    pub(super) fn compile(&self) -> Result<WeaponComponentGraphDefinition, String> {
        let graph = WeaponComponentGraphDefinition {
            points: self
                .points
                .iter()
                .map(|point| WeaponComponentPointDefinition {
                    id: point.id.clone(),
                    attach_joint: point.attach_joint.clone(),
                    allowed_components: point.allowed_components.clone(),
                })
                .collect(),
            components: self
                .components
                .iter()
                .map(|component| -> Result<_, String> {
                    let id = component.id.trim().to_ascii_lowercase();
                    Ok((
                        id.clone(),
                        WeaponComponentDefinition {
                            id,
                            slot: component.slot.clone(),
                            model_ref: (!component.model_ref.trim().is_empty())
                                .then(|| component.model_ref.clone()),
                            audio_override: (!component.audio_override.trim().is_empty())
                                .then(|| component.audio_override.clone()),
                            muzzle_vfx_override: (!component.muzzle_vfx_override.trim().is_empty())
                                .then(|| component.muzzle_vfx_override.clone()),
                            tracer_vfx_override: (!component.tracer_vfx_override.trim().is_empty())
                                .then(|| component.tracer_vfx_override.clone()),
                            stat_modifiers: crate::weapon_profiles::compile_weapon_stat_stack(
                                &component.stat_modifiers,
                            )?,
                            modifiers: component.modifiers.compile(),
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
            default_installed: self.default_installed.clone(),
        }
        .sanitized();
        graph.validate()?;
        Ok(graph)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponDefinition {
    #[serde(rename = "type", alias = "weapon_type")]
    pub weapon_type: String,
    /// Open-ended authored presentation family. This is intentionally not a gameplay enum.
    pub class: String,
    pub rank: Option<u16>,
    pub ammo: String,
    pub fire_mode: String,
    pub firing_pattern_kind: String,
    pub bursts_min: u8,
    pub bursts_max: u8,
    pub shots_per_burst_min: u8,
    pub shots_per_burst_max: u8,
    pub time_between_shots: f32,
    pub time_between_bursts: f32,
    pub delay_before_firing: f32,
    pub burst_cooldown: f32,
    pub magazine_capacity: u32,
    pub reserve_capacity: u32,
    pub fire_interval: f32,
    pub reload_duration: f32,
    pub damage: f32,
    pub range: f32,
    pub hip_spread_degrees: f32,
    pub aim_spread_degrees: f32,
    pub movement_spread_multiplier: f32,
    pub crouch_spread_multiplier: f32,
    pub recoil_accuracy_per_shot_degrees: f32,
    pub recoil_accuracy_max_degrees: f32,
    pub accuracy_recovery_hz: f32,
    pub accuracy_recovery_delay_seconds: f32,
    pub recoil_pitch_degrees: f32,
    pub recoil_pitch_random_degrees: f32,
    pub recoil_yaw_degrees: f32,
    pub recoil_yaw_bias_degrees: f32,
    pub ads_recoil_multiplier: f32,
    pub recoil_recovery_hz: f32,
    pub recoil_pitch_tracker_speed_scale: f32,
    pub recoil_yaw_tracker_speed_scale: f32,
    pub muzzle_forward_offset: f32,
    pub ricochet_enabled: bool,
    pub ricochet_max_bounces: u8,
    pub ricochet_grazing_dot: f32,
    pub ricochet_energy_retention: f32,
    /// Optional V2 authored profile decomposition. When absent, the runtime projects the legacy
    /// flat fields into equivalent handling/spread/recoil/sway/ADS profiles.
    pub profiles: Option<AuthoredWeaponRuntimeProfiles>,
    pub melee_damage: f32,
    pub melee_range: f32,
    pub melee_attack_interval: f32,
}

impl Default for AuthoredWeaponDefinition {
    fn default() -> Self {
        let tuning = HitscanWeaponTuning::default();
        let melee = MeleeWeaponTuning::default();
        Self {
            weapon_type: "firearm".to_owned(),
            class: String::new(),
            rank: None,
            ammo: String::new(),
            fire_mode: "semi_auto".to_owned(),
            firing_pattern_kind: String::new(),
            bursts_min: 1,
            bursts_max: 1,
            shots_per_burst_min: 1,
            shots_per_burst_max: 1,
            time_between_shots: tuning.fire_interval,
            time_between_bursts: 0.0,
            delay_before_firing: 0.0,
            burst_cooldown: 0.0,
            magazine_capacity: tuning.magazine_capacity,
            reserve_capacity: tuning.reserve_capacity,
            fire_interval: tuning.fire_interval,
            reload_duration: tuning.reload_duration,
            damage: tuning.damage,
            range: tuning.range,
            hip_spread_degrees: tuning.hip_spread_radians.to_degrees(),
            aim_spread_degrees: tuning.aim_spread_radians.to_degrees(),
            movement_spread_multiplier: tuning.movement_spread_multiplier,
            crouch_spread_multiplier: tuning.crouch_spread_multiplier,
            recoil_accuracy_per_shot_degrees: tuning.recoil_accuracy_per_shot_radians.to_degrees(),
            recoil_accuracy_max_degrees: tuning.recoil_accuracy_max_radians.to_degrees(),
            accuracy_recovery_hz: tuning.accuracy_recovery_hz,
            accuracy_recovery_delay_seconds: tuning.accuracy_recovery_delay_seconds,
            recoil_pitch_degrees: tuning.recoil_pitch_radians.to_degrees(),
            recoil_pitch_random_degrees: tuning.recoil_pitch_random_radians.to_degrees(),
            recoil_yaw_degrees: tuning.recoil_yaw_radians.to_degrees(),
            recoil_yaw_bias_degrees: tuning.recoil_yaw_bias_radians.to_degrees(),
            ads_recoil_multiplier: tuning.ads_recoil_multiplier,
            recoil_recovery_hz: tuning.recoil_recovery_hz,
            recoil_pitch_tracker_speed_scale: tuning.recoil_pitch_tracker_speed_scale,
            recoil_yaw_tracker_speed_scale: tuning.recoil_yaw_tracker_speed_scale,
            muzzle_forward_offset: tuning.muzzle_forward_offset,
            ricochet_enabled: tuning.ricochet_enabled,
            ricochet_max_bounces: tuning.ricochet_max_bounces,
            ricochet_grazing_dot: tuning.ricochet_grazing_dot,
            ricochet_energy_retention: tuning.ricochet_energy_retention,
            profiles: None,
            melee_damage: melee.damage,
            melee_range: melee.range,
            melee_attack_interval: melee.attack_interval,
        }
    }
}

impl AuthoredWeaponDefinition {
    pub(super) fn weapon_type(&self) -> Result<WeaponType, String> {
        match self
            .weapon_type
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "unarmed" | "fists" => Ok(WeaponType::Unarmed),
            "melee" => Ok(WeaponType::Melee),
            "firearm" | "gun" => Ok(WeaponType::Firearm),
            other => Err(format!("unsupported weapon type '{other}'")),
        }
    }

    pub(super) fn effective_rank(&self, weapon_type: WeaponType) -> u16 {
        self.rank.unwrap_or_else(|| weapon_type.default_rank())
    }

    pub(super) fn melee_tuning(&self) -> MeleeWeaponTuning {
        MeleeWeaponTuning {
            damage: self.melee_damage,
            range: self.melee_range,
            attack_interval: self.melee_attack_interval,
        }
        .sanitized()
    }

    pub(super) fn fire_mode(&self) -> Result<WeaponFireMode, String> {
        match self
            .fire_mode
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "" | "semi" | "semi_auto" | "semiauto" => Ok(WeaponFireMode::SemiAuto),
            "auto" | "automatic" | "full_auto" | "fullauto" => Ok(WeaponFireMode::Automatic),
            other => Err(format!("unsupported weapon fire_mode '{other}'")),
        }
    }

    pub(super) fn firing_pattern(
        &self,
        fire_mode: WeaponFireMode,
    ) -> Result<FiringPatternDefinition, String> {
        let kind = match self
            .firing_pattern_kind
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "" => {
                return Ok(FiringPatternDefinition::from_fire_mode(
                    fire_mode,
                    self.fire_interval,
                ))
            }
            "semi" | "semi_auto" | "semiauto" => FiringPatternKind::Semi,
            "auto" | "automatic" | "full_auto" => FiringPatternKind::Automatic,
            "burst" => FiringPatternKind::Burst,
            "charge" => FiringPatternKind::Charge,
            "spin_up" | "spinup" => FiringPatternKind::SpinUp,
            "pump" => FiringPatternKind::Pump,
            "bolt_action" | "boltaction" => FiringPatternKind::BoltAction,
            "binary" => FiringPatternKind::Binary,
            "scripted_sequence" | "scripted" => FiringPatternKind::ScriptedSequence,
            other => return Err(format!("unsupported firing_pattern_kind '{other}'")),
        };
        Ok(FiringPatternDefinition {
            kind,
            bursts_min: self.bursts_min,
            bursts_max: self.bursts_max,
            shots_per_burst_min: self.shots_per_burst_min,
            shots_per_burst_max: self.shots_per_burst_max,
            time_between_shots: self.time_between_shots,
            time_between_bursts: self.time_between_bursts,
            delay_before_firing: self.delay_before_firing,
            burst_cooldown: self.burst_cooldown,
        }
        .sanitized())
    }

    pub(super) fn runtime_profiles(&self) -> Result<WeaponRuntimeProfiles, String> {
        let tuning = self.tuning();
        self.profiles
            .as_ref()
            .map(|profiles| profiles.compile(tuning))
            .unwrap_or_else(|| Ok(WeaponRuntimeProfiles::from_legacy_tuning(tuning)))
    }

    pub(super) fn tuning(&self) -> HitscanWeaponTuning {
        HitscanWeaponTuning {
            magazine_capacity: self.magazine_capacity,
            reserve_capacity: self.reserve_capacity,
            fire_interval: self.fire_interval,
            reload_duration: self.reload_duration,
            damage: self.damage,
            range: self.range,
            hip_spread_radians: self.hip_spread_degrees.to_radians(),
            aim_spread_radians: self.aim_spread_degrees.to_radians(),
            movement_spread_multiplier: self.movement_spread_multiplier,
            crouch_spread_multiplier: self.crouch_spread_multiplier,
            recoil_accuracy_per_shot_radians: self.recoil_accuracy_per_shot_degrees.to_radians(),
            recoil_accuracy_max_radians: self.recoil_accuracy_max_degrees.to_radians(),
            accuracy_recovery_hz: self.accuracy_recovery_hz,
            accuracy_recovery_delay_seconds: self.accuracy_recovery_delay_seconds,
            recoil_pitch_radians: self.recoil_pitch_degrees.to_radians(),
            recoil_pitch_random_radians: self.recoil_pitch_random_degrees.to_radians(),
            recoil_yaw_radians: self.recoil_yaw_degrees.to_radians(),
            recoil_yaw_bias_radians: self.recoil_yaw_bias_degrees.to_radians(),
            ads_recoil_multiplier: self.ads_recoil_multiplier,
            recoil_recovery_hz: self.recoil_recovery_hz,
            recoil_pitch_tracker_speed_scale: self.recoil_pitch_tracker_speed_scale,
            recoil_yaw_tracker_speed_scale: self.recoil_yaw_tracker_speed_scale,
            muzzle_forward_offset: self.muzzle_forward_offset,
            ricochet_enabled: self.ricochet_enabled,
            ricochet_max_bounces: self.ricochet_max_bounces,
            ricochet_grazing_dot: self.ricochet_grazing_dot,
            ricochet_energy_retention: self.ricochet_energy_retention,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponAnimationDefinition {
    pub skeleton: String,
    pub animation_dictionary: String,
    pub idle: String,
    pub fire: String,
    pub reload: String,
    pub spawn_pose: String,
}

impl AuthoredWeaponAnimationDefinition {
    pub(super) fn compile(&self) -> WeaponAnimationDefinition {
        fn clip(value: &str) -> Option<String> {
            let value = value.trim().replace('\\', "/");
            (!value.is_empty()).then_some(value)
        }
        WeaponAnimationDefinition {
            skeleton: clip(&self.skeleton),
            animation_dictionary: clip(&self.animation_dictionary),
            idle: clip(&self.idle),
            fire: clip(&self.fire),
            reload: clip(&self.reload),
            spawn_pose: clip(&self.spawn_pose),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponPresentationDefinition {
    pub enabled: bool,
    pub handle_from_root: [f32; 3],
    pub muzzle_from_root: [f32; 3],
    pub left_grip_from_handle: [f32; 3],
    pub stock_contact_from_handle: [f32; 3],
    pub ready_shoulder_pocket_offset: [f32; 3],
    pub ads_shoulder_pocket_offset: [f32; 3],
    pub fire_kick_duration_seconds: f32,
    pub fire_kick_pitch_radians: f32,
    /// Third-person ReadyHold body -> weapon native-rig rotation; runtime basis completion is
    /// carried separately by `native_rig_to_runtime_basis`.
    pub ready_body_to_root_rotation: [f32; 4],
    pub ready_right_elbow_pole_offset: [f32; 3],
    pub ready_left_elbow_pole_offset: [f32; 3],
    pub ready_left_palm_to_left_grip: [f32; 3],
    pub ready_right_palm_to_weapon: [f32; 4],
    pub ready_left_palm_to_weapon: [f32; 4],
    pub right_palm_to_handle: [f32; 3],
    pub right_palm_to_native_rig: [f32; 4],
    /// Authored native-rig -> runtime basis correction for weapon-space offsets and orientation.
    pub native_rig_to_runtime_basis: [f32; 4],
    pub first_person_hip_handle_offset: [f32; 3],
    pub first_person_full_body_hip_handle_offset: Option<[f32; 3]>,
    pub ads_rear_sight_from_handle: [f32; 3],
    pub ads_front_sight_from_handle: [f32; 3],
    pub ads_camera_to_rear_sight: [f32; 3],
    pub ads_camera_translation_weight: [f32; 3],
    pub first_person_hip_convergence_m: f32,
    pub aim_response_hz: f32,
    pub secondary_hip_max_angle_radians: f32,
    pub secondary_ads_max_angle_radians: f32,
    pub secondary_angular_inertia_gain: f32,
    pub secondary_movement_inertia_gain: f32,
    pub secondary_natural_hz_hip: f32,
    pub secondary_natural_hz_ads: f32,
    pub secondary_obstruction_hz_boost: f32,
}

impl Default for AuthoredWeaponPresentationDefinition {
    fn default() -> Self {
        let runtime = WeaponPresentationDefinition::default();
        Self {
            enabled: runtime.enabled,
            handle_from_root: runtime.handle_from_root,
            muzzle_from_root: runtime.muzzle_from_root,
            left_grip_from_handle: runtime.left_grip_from_handle,
            stock_contact_from_handle: runtime.stock_contact_from_handle,
            ready_shoulder_pocket_offset: runtime.ready_shoulder_pocket_offset,
            ads_shoulder_pocket_offset: runtime.ads_shoulder_pocket_offset,
            fire_kick_duration_seconds: runtime.fire_kick_duration_seconds,
            fire_kick_pitch_radians: runtime.fire_kick_pitch_radians,
            ready_body_to_root_rotation: runtime.ready_body_to_root_rotation,
            ready_right_elbow_pole_offset: runtime.ready_right_elbow_pole_offset,
            ready_left_elbow_pole_offset: runtime.ready_left_elbow_pole_offset,
            ready_left_palm_to_left_grip: runtime.ready_left_palm_to_left_grip,
            ready_right_palm_to_weapon: runtime.ready_right_palm_to_weapon,
            ready_left_palm_to_weapon: runtime.ready_left_palm_to_weapon,
            right_palm_to_handle: runtime.right_palm_to_handle,
            right_palm_to_native_rig: runtime.right_palm_to_native_rig,
            native_rig_to_runtime_basis: runtime.native_rig_to_runtime_basis,
            first_person_hip_handle_offset: runtime.first_person_hip_handle_offset,
            first_person_full_body_hip_handle_offset: None,
            ads_rear_sight_from_handle: runtime.ads_rear_sight_from_handle,
            ads_front_sight_from_handle: runtime.ads_front_sight_from_handle,
            ads_camera_to_rear_sight: runtime.ads_camera_to_rear_sight,
            ads_camera_translation_weight: runtime.ads_camera_translation_weight,
            first_person_hip_convergence_m: runtime.first_person_hip_convergence_m,
            aim_response_hz: runtime.aim_response_hz,
            secondary_hip_max_angle_radians: runtime.secondary_hip_max_angle_radians,
            secondary_ads_max_angle_radians: runtime.secondary_ads_max_angle_radians,
            secondary_angular_inertia_gain: runtime.secondary_angular_inertia_gain,
            secondary_movement_inertia_gain: runtime.secondary_movement_inertia_gain,
            secondary_natural_hz_hip: runtime.secondary_natural_hz_hip,
            secondary_natural_hz_ads: runtime.secondary_natural_hz_ads,
            secondary_obstruction_hz_boost: runtime.secondary_obstruction_hz_boost,
        }
    }
}

impl AuthoredWeaponPresentationDefinition {
    pub(super) fn compile(&self) -> WeaponPresentationDefinition {
        WeaponPresentationDefinition {
            enabled: self.enabled,
            handle_from_root: self.handle_from_root,
            muzzle_from_root: self.muzzle_from_root,
            left_grip_from_handle: self.left_grip_from_handle,
            stock_contact_from_handle: self.stock_contact_from_handle,
            ready_shoulder_pocket_offset: self.ready_shoulder_pocket_offset,
            ads_shoulder_pocket_offset: self.ads_shoulder_pocket_offset,
            fire_kick_duration_seconds: self.fire_kick_duration_seconds,
            fire_kick_pitch_radians: self.fire_kick_pitch_radians,
            ready_body_to_root_rotation: self.ready_body_to_root_rotation,
            ready_right_elbow_pole_offset: self.ready_right_elbow_pole_offset,
            ready_left_elbow_pole_offset: self.ready_left_elbow_pole_offset,
            ready_left_palm_to_left_grip: self.ready_left_palm_to_left_grip,
            ready_right_palm_to_weapon: self.ready_right_palm_to_weapon,
            ready_left_palm_to_weapon: self.ready_left_palm_to_weapon,
            right_palm_to_handle: self.right_palm_to_handle,
            right_palm_to_native_rig: self.right_palm_to_native_rig,
            native_rig_to_runtime_basis: self.native_rig_to_runtime_basis,
            first_person_hip_handle_offset: self.first_person_hip_handle_offset,
            first_person_full_body_hip_handle_offset: self
                .first_person_full_body_hip_handle_offset
                .unwrap_or(self.first_person_hip_handle_offset),
            ads_rear_sight_from_handle: self.ads_rear_sight_from_handle,
            ads_front_sight_from_handle: self.ads_front_sight_from_handle,
            ads_camera_to_rear_sight: self.ads_camera_to_rear_sight,
            ads_camera_translation_weight: self.ads_camera_translation_weight,
            first_person_hip_convergence_m: self.first_person_hip_convergence_m,
            aim_response_hz: self.aim_response_hz,
            secondary_hip_max_angle_radians: self.secondary_hip_max_angle_radians,
            secondary_ads_max_angle_radians: self.secondary_ads_max_angle_radians,
            secondary_angular_inertia_gain: self.secondary_angular_inertia_gain,
            secondary_movement_inertia_gain: self.secondary_movement_inertia_gain,
            secondary_natural_hz_hip: self.secondary_natural_hz_hip,
            secondary_natural_hz_ads: self.secondary_natural_hz_ads,
            secondary_obstruction_hz_boost: self.secondary_obstruction_hz_boost,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponCasingDefinition {
    pub model_dictionary: String,
    pub variants: Vec<String>,
    pub material_ref: String,
    pub half_extents: [f32; 3],
    pub ejection_delay_seconds: f32,
    pub ejection_joint: String,
    pub inherit_socket_linear_velocity: f32,
    pub inherit_socket_angular_velocity: f32,
    /// Local `[right, up, forward]` coefficients relative to the authored ejection socket.
    pub origin_local: [f32; 3],
    pub velocity_local: [f32; 3],
    pub velocity_jitter: [f32; 3],
    pub axis_local: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub angular_velocity_jitter: [f32; 3],
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub contact_min_impulse: f32,
    pub contact_medium_impulse: f32,
    pub contact_hard_impulse: f32,
    pub soft_surface_contains: Vec<String>,
}

impl Default for AuthoredWeaponCasingDefinition {
    fn default() -> Self {
        let runtime = WeaponCasingDefinition::default();
        Self {
            model_dictionary: String::new(),
            variants: Vec::new(),
            material_ref: String::new(),
            half_extents: runtime.half_extents,
            ejection_delay_seconds: runtime.ejection_delay_seconds,
            ejection_joint: runtime.ejection_joint.unwrap_or_default(),
            inherit_socket_linear_velocity: runtime.inherit_socket_linear_velocity,
            inherit_socket_angular_velocity: runtime.inherit_socket_angular_velocity,
            origin_local: runtime.origin_local,
            velocity_local: runtime.velocity_local,
            velocity_jitter: runtime.velocity_jitter,
            axis_local: runtime.axis_local,
            angular_velocity: runtime.angular_velocity,
            angular_velocity_jitter: runtime.angular_velocity_jitter,
            friction: runtime.friction,
            restitution: runtime.restitution,
            density: runtime.density,
            linear_damping: runtime.linear_damping,
            angular_damping: runtime.angular_damping,
            contact_min_impulse: runtime.contact_min_impulse,
            contact_medium_impulse: runtime.contact_medium_impulse,
            contact_hard_impulse: runtime.contact_hard_impulse,
            soft_surface_contains: runtime.soft_surface_contains,
        }
    }
}

impl AuthoredWeaponCasingDefinition {
    pub(super) fn compile(&self) -> WeaponCasingDefinition {
        WeaponCasingDefinition {
            model_dictionary: (!self.model_dictionary.trim().is_empty())
                .then(|| self.model_dictionary.trim().replace('\\', "/")),
            variants: self.variants.clone(),
            material_ref: (!self.material_ref.trim().is_empty())
                .then(|| self.material_ref.trim().replace('\\', "/")),
            half_extents: self.half_extents,
            ejection_delay_seconds: self.ejection_delay_seconds,
            ejection_joint: (!self.ejection_joint.trim().is_empty())
                .then(|| self.ejection_joint.trim().to_owned()),
            inherit_socket_linear_velocity: self.inherit_socket_linear_velocity,
            inherit_socket_angular_velocity: self.inherit_socket_angular_velocity,
            origin_local: self.origin_local,
            velocity_local: self.velocity_local,
            velocity_jitter: self.velocity_jitter,
            axis_local: self.axis_local,
            angular_velocity: self.angular_velocity,
            angular_velocity_jitter: self.angular_velocity_jitter,
            friction: self.friction,
            restitution: self.restitution,
            density: self.density,
            linear_damping: self.linear_damping,
            angular_damping: self.angular_damping,
            contact_min_impulse: self.contact_min_impulse,
            contact_medium_impulse: self.contact_medium_impulse,
            contact_hard_impulse: self.contact_hard_impulse,
            soft_surface_contains: self.soft_surface_contains.clone(),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponVfxDefinition {
    pub shot: String,
    pub tracer: String,
    pub ricochet: String,
    pub exit: String,
    pub impact_default: String,
    pub impact_by_surface: std::collections::BTreeMap<String, String>,
}

impl AuthoredWeaponVfxDefinition {
    pub(super) fn compile(&self) -> WeaponVfxDefinition {
        WeaponVfxDefinition {
            shot: (!self.shot.trim().is_empty()).then(|| self.shot.trim().to_owned()),
            tracer: (!self.tracer.trim().is_empty()).then(|| self.tracer.trim().to_owned()),
            ricochet: (!self.ricochet.trim().is_empty()).then(|| self.ricochet.trim().to_owned()),
            exit: (!self.exit.trim().is_empty()).then(|| self.exit.trim().to_owned()),
            impact_default: (!self.impact_default.trim().is_empty())
                .then(|| self.impact_default.trim().to_owned()),
            impact_by_surface: self.impact_by_surface.clone(),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponAudioDefinition {
    pub fire: String,
    pub reload_start: String,
    pub reload_complete: String,
    pub equip: String,
    pub unequip: String,
    pub empty: String,
    pub shell_eject: String,
    pub shell_contact_small: String,
    pub shell_contact_medium: String,
    pub shell_contact_hard: String,
    pub shell_contact_soft: String,
}

impl AuthoredWeaponAudioDefinition {
    pub(super) fn compile(&self) -> WeaponAudioDefinition {
        fn clip(value: &str) -> Option<String> {
            let value = value.trim().replace('\\', "/");
            (!value.is_empty()).then_some(value)
        }
        WeaponAudioDefinition {
            fire: clip(&self.fire),
            reload_start: clip(&self.reload_start),
            reload_complete: clip(&self.reload_complete),
            equip: clip(&self.equip),
            unequip: clip(&self.unequip),
            empty: clip(&self.empty),
            shell_eject: clip(&self.shell_eject),
            shell_contact_small: clip(&self.shell_contact_small),
            shell_contact_medium: clip(&self.shell_contact_medium),
            shell_contact_hard: clip(&self.shell_contact_hard),
            shell_contact_soft: clip(&self.shell_contact_soft),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredUseEffect {
    pub kind: String,
    pub amount: f32,
}

impl Default for AuthoredUseEffect {
    fn default() -> Self {
        Self {
            kind: "none".to_owned(),
            amount: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWorldItemDefinition {
    pub model: String,
    pub material_library: String,
    pub fallback_primitive: String,
    pub scale: [f32; 3],
    pub color_rgba: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl Default for AuthoredWorldItemDefinition {
    fn default() -> Self {
        Self {
            model: String::new(),
            material_library: String::new(),
            fallback_primitive: "cube".to_owned(),
            scale: [0.2, 0.2, 0.2],
            color_rgba: [0.55, 0.60, 0.68, 1.0],
            pickup_half_extents: [0.2, 0.2, 0.2],
            respawn_seconds: 0.0,
        }
    }
}

impl AuthoredWorldItemDefinition {
    pub(super) fn compile(&self, kind: ItemKind) -> Result<WorldItemDefinition, String> {
        let fallback_primitive = match self.fallback_primitive.trim().to_ascii_lowercase().as_str()
        {
            "" | "cube" => primitive_builtins::ID_CUBE,
            "sphere" | "sphere_uv" => primitive_builtins::ID_SPHERE_UV,
            "cylinder" => primitive_builtins::ID_CYLINDER,
            "capsule" => primitive_builtins::ID_CAPSULE,
            "cone" => primitive_builtins::ID_CONE,
            "torus" => primitive_builtins::ID_TORUS,
            "disc" => primitive_builtins::ID_DISC,
            other => return Err(format!("unsupported world fallback primitive '{other}'")),
        };
        let mut definition = WorldItemDefinition::for_kind(kind);
        definition.model_ref =
            (!self.model.trim().is_empty()).then(|| self.model.trim().to_owned());
        definition.material_library_ref = (!self.material_library.trim().is_empty())
            .then(|| self.material_library.trim().to_owned());
        definition.fallback_primitive = fallback_primitive;
        definition.scale = self.scale;
        definition.color = self.color_rgba;
        definition.pickup_half_extents = self.pickup_half_extents;
        definition.respawn_seconds = self.respawn_seconds;
        Ok(definition.sanitized())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredLoadoutDefinition {
    pub id: String,
    pub display_name: String,
    pub clear_existing: bool,
    pub entries: Vec<AuthoredLoadoutEntry>,
}

impl Default for AuthoredLoadoutDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            clear_existing: true,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredLoadoutEntry {
    pub item: String,
    pub quantity: u32,
    pub equip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledItemPackage {
    pub catalog: ItemCatalog,
    pub loadouts: InventoryLoadoutCatalog,
}
