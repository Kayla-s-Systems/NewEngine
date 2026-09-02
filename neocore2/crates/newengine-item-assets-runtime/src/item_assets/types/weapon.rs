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
    pub reload_topology: String,
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
            reload_topology: "detachable_magazine".to_owned(),
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
        let mut profiles = self
            .profiles
            .as_ref()
            .map(|profiles| profiles.compile(tuning))
            .unwrap_or_else(|| Ok(WeaponRuntimeProfiles::from_legacy_tuning(tuning)))?;
        if self
            .profiles
            .as_ref()
            .and_then(|profiles| profiles.handling.reload_topology.as_ref())
            .is_none()
        {
            profiles.handling.reload_topology =
                crate::weapon_profiles::parse_reload_topology(&self.reload_topology)?;
        }
        Ok(profiles.sanitized())
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
