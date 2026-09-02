use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponRuntimeProfiles {
    pub handling: AuthoredWeaponHandlingProfile,
    pub spread: AuthoredWeaponSpreadProfile,
    pub recoil: AuthoredWeaponRecoilProfile,
    pub sway: AuthoredWeaponSwayProfile,
    pub ads: AuthoredWeaponAdsProfile,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponHandlingProfile {
    pub reload_duration_seconds: Option<f32>,
    pub reload_topology: Option<String>,
    pub equip_duration_seconds: Option<f32>,
    pub unequip_duration_seconds: Option<f32>,
    pub aim_in_duration_seconds: Option<f32>,
    pub aim_out_duration_seconds: Option<f32>,
    pub muzzle_forward_offset: Option<f32>,
    pub reload_magazine_detach_fraction: Option<f32>,
    pub reload_ammo_commit_fraction: Option<f32>,
    pub reload_magazine_insert_fraction: Option<f32>,
    pub reload_chamber_fraction: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponSpreadStateProfile {
    pub default_degrees: Option<[f32; 2]>,
    pub minimum_degrees: Option<[f32; 2]>,
    pub maximum_degrees: Option<[f32; 2]>,
    pub change_per_shot_degrees: Option<[f32; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponSpreadProfile {
    pub hip: AuthoredWeaponSpreadStateProfile,
    pub ads: AuthoredWeaponSpreadStateProfile,
    pub distribution: Option<String>,
    pub movement_multiplier: Option<f32>,
    pub crouch_multiplier: Option<f32>,
    pub recovery_hz: Option<f32>,
    pub recovery_delay_seconds: Option<f32>,
    pub zero_first_shot: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponRecoilStateProfile {
    pub pitch_degrees: Option<f32>,
    pub pitch_random_degrees: Option<f32>,
    pub yaw_degrees: Option<f32>,
    pub yaw_bias_degrees: Option<f32>,
    pub recovery_hz: Option<f32>,
    pub hold_seconds: Option<f32>,
    pub max_accumulated_degrees: Option<f32>,
    pub pitch_tracker_speed_scale: Option<f32>,
    pub yaw_tracker_speed_scale: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponRecoilProfile {
    pub hip: AuthoredWeaponRecoilStateProfile,
    pub ads: AuthoredWeaponRecoilStateProfile,
    pub drift_min_degrees: Option<f32>,
    pub drift_max_degrees: Option<f32>,
    pub drift_full_kick_degrees: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponSwayProfile {
    pub traversal_seconds: Option<f32>,
    pub horizontal_degrees: Option<f32>,
    pub vertical_degrees: Option<f32>,
    pub start_delay_seconds: Option<f32>,
    pub blend_in_seconds: Option<f32>,
    pub ads_multiplier: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponAdsProfile {
    pub recoil_multiplier: Option<f32>,
    pub fov_multiplier: Option<f32>,
    pub sensitivity_multiplier: Option<f32>,
}

impl AuthoredWeaponRuntimeProfiles {
    pub(super) fn compile(
        &self,
        legacy_tuning: HitscanWeaponTuning,
    ) -> Result<WeaponRuntimeProfiles, String> {
        let mut profiles = WeaponRuntimeProfiles::from_legacy_tuning(legacy_tuning);

        apply_opt(
            &mut profiles.handling.reload_duration_seconds,
            self.handling.reload_duration_seconds,
        );
        if let Some(topology) = self.handling.reload_topology.as_deref() {
            profiles.handling.reload_topology = parse_reload_topology(topology)?;
        }
        apply_opt(
            &mut profiles.handling.equip_duration_seconds,
            self.handling.equip_duration_seconds,
        );
        apply_opt(
            &mut profiles.handling.unequip_duration_seconds,
            self.handling.unequip_duration_seconds,
        );
        apply_opt(
            &mut profiles.handling.aim_in_duration_seconds,
            self.handling.aim_in_duration_seconds,
        );
        apply_opt(
            &mut profiles.handling.aim_out_duration_seconds,
            self.handling.aim_out_duration_seconds,
        );
        apply_opt(
            &mut profiles.handling.muzzle_forward_offset,
            self.handling.muzzle_forward_offset,
        );
        apply_opt(
            &mut profiles.handling.reload_timeline.magazine_detach_fraction,
            self.handling.reload_magazine_detach_fraction,
        );
        apply_opt(
            &mut profiles.handling.reload_timeline.ammo_commit_fraction,
            self.handling.reload_ammo_commit_fraction,
        );
        apply_opt(
            &mut profiles.handling.reload_timeline.magazine_insert_fraction,
            self.handling.reload_magazine_insert_fraction,
        );
        apply_opt(
            &mut profiles.handling.reload_timeline.chamber_fraction,
            self.handling.reload_chamber_fraction,
        );

        apply_spread_state(&mut profiles.spread.hip, &self.spread.hip);
        apply_spread_state(&mut profiles.spread.ads, &self.spread.ads);
        if let Some(distribution) = self.spread.distribution.as_deref() {
            profiles.spread.distribution = parse_spread_distribution(distribution)?;
        }
        apply_opt(
            &mut profiles.spread.movement_multiplier,
            self.spread.movement_multiplier,
        );
        apply_opt(
            &mut profiles.spread.crouch_multiplier,
            self.spread.crouch_multiplier,
        );
        apply_opt(&mut profiles.spread.recovery_hz, self.spread.recovery_hz);
        apply_opt(
            &mut profiles.spread.recovery_delay_seconds,
            self.spread.recovery_delay_seconds,
        );
        if let Some(value) = self.spread.zero_first_shot {
            profiles.spread.zero_first_shot = value;
        }

        apply_recoil_state(&mut profiles.recoil.hip, &self.recoil.hip);
        apply_recoil_state(&mut profiles.recoil.ads, &self.recoil.ads);
        apply_degrees(
            &mut profiles.recoil.drift_min_radians,
            self.recoil.drift_min_degrees,
        );
        apply_degrees(
            &mut profiles.recoil.drift_max_radians,
            self.recoil.drift_max_degrees,
        );
        apply_degrees(
            &mut profiles.recoil.drift_full_kick_radians,
            self.recoil.drift_full_kick_degrees,
        );

        apply_opt(
            &mut profiles.sway.traversal_seconds,
            self.sway.traversal_seconds,
        );
        apply_degrees(
            &mut profiles.sway.horizontal_radians,
            self.sway.horizontal_degrees,
        );
        apply_degrees(
            &mut profiles.sway.vertical_radians,
            self.sway.vertical_degrees,
        );
        apply_opt(
            &mut profiles.sway.start_delay_seconds,
            self.sway.start_delay_seconds,
        );
        apply_opt(
            &mut profiles.sway.blend_in_seconds,
            self.sway.blend_in_seconds,
        );
        apply_opt(&mut profiles.sway.ads_multiplier, self.sway.ads_multiplier);

        apply_opt(
            &mut profiles.ads.recoil_multiplier,
            self.ads.recoil_multiplier,
        );
        apply_opt(&mut profiles.ads.fov_multiplier, self.ads.fov_multiplier);
        apply_opt(
            &mut profiles.ads.sensitivity_multiplier,
            self.ads.sensitivity_multiplier,
        );

        Ok(profiles.sanitized())
    }
}

fn apply_spread_state(
    target: &mut WeaponSpreadStateProfile,
    authored: &AuthoredWeaponSpreadStateProfile,
) {
    if let Some(value) = authored.default_degrees {
        target.default_radians = value.map(f32::to_radians);
    }
    if let Some(value) = authored.minimum_degrees {
        target.minimum_radians = value.map(f32::to_radians);
    }
    if let Some(value) = authored.maximum_degrees {
        target.maximum_radians = value.map(f32::to_radians);
    }
    if let Some(value) = authored.change_per_shot_degrees {
        target.change_per_shot_radians = value.map(f32::to_radians);
    }
}

fn apply_recoil_state(
    target: &mut WeaponRecoilStateProfile,
    authored: &AuthoredWeaponRecoilStateProfile,
) {
    apply_degrees(&mut target.pitch_radians, authored.pitch_degrees);
    apply_degrees(
        &mut target.pitch_random_radians,
        authored.pitch_random_degrees,
    );
    apply_degrees(&mut target.yaw_radians, authored.yaw_degrees);
    apply_degrees(&mut target.yaw_bias_radians, authored.yaw_bias_degrees);
    apply_opt(&mut target.recovery_hz, authored.recovery_hz);
    apply_opt(&mut target.hold_seconds, authored.hold_seconds);
    apply_degrees(
        &mut target.max_accumulated_radians,
        authored.max_accumulated_degrees,
    );
    apply_opt(
        &mut target.pitch_tracker_speed_scale,
        authored.pitch_tracker_speed_scale,
    );
    apply_opt(
        &mut target.yaw_tracker_speed_scale,
        authored.yaw_tracker_speed_scale,
    );
}

#[inline]
fn apply_opt(target: &mut f32, value: Option<f32>) {
    if let Some(value) = value {
        *target = value;
    }
}

#[inline]
fn apply_degrees(target: &mut f32, value: Option<f32>) {
    if let Some(value) = value {
        *target = value.to_radians();
    }
}

pub(super) fn parse_reload_topology(value: &str) -> Result<WeaponReloadTopology, String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "" | "detachable" | "detachable_mag" | "detachable_magazine" | "magazine" => {
            Ok(WeaponReloadTopology::DetachableMagazine)
        }
        "internal" | "internal_mag" | "internal_magazine" => {
            Ok(WeaponReloadTopology::InternalMagazine)
        }
        "single" | "single_round" | "single_round_chamber" => Ok(WeaponReloadTopology::SingleRound),
        other => Err(format!("unsupported weapon reload topology '{other}'")),
    }
}

fn parse_spread_distribution(value: &str) -> Result<WeaponSpreadDistribution, String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "" | "rect" | "rectangle" | "rectangular" => Ok(WeaponSpreadDistribution::Rectangular),
        "circle" | "circular" | "disk" | "uniform_disk" => Ok(WeaponSpreadDistribution::Circular),
        "gaussian" | "normal" => Ok(WeaponSpreadDistribution::Gaussian),
        "even" | "even_jitter" | "grid_jitter" => Ok(WeaponSpreadDistribution::EvenJitter),
        other => Err(format!("unsupported weapon spread distribution '{other}'")),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponStatModifier {
    pub stat: String,
    pub operation: String,
    pub value: f32,
    pub priority: i16,
}

impl Default for AuthoredWeaponStatModifier {
    fn default() -> Self {
        Self {
            stat: String::new(),
            operation: "multiply".to_owned(),
            value: 1.0,
            priority: 0,
        }
    }
}

impl AuthoredWeaponStatModifier {
    pub(super) fn compile(&self) -> Result<WeaponStatModifier, String> {
        let stat = match self
            .stat
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "damage" | "damage_multiplier" => WeaponStatId::DamageMultiplier,
            "recoil" | "recoil_multiplier" => WeaponStatId::RecoilMultiplier,
            "spread" | "accuracy" | "spread_multiplier" | "accuracy_multiplier" => {
                WeaponStatId::SpreadMultiplier
            }
            "reload" | "reload_duration" | "reload_duration_multiplier" => {
                WeaponStatId::ReloadDurationMultiplier
            }
            "muzzle_velocity" | "muzzle_velocity_multiplier" => {
                WeaponStatId::MuzzleVelocityMultiplier
            }
            "penetration" | "penetration_multiplier" => WeaponStatId::PenetrationMultiplier,
            "falloff" | "falloff_multiplier" => WeaponStatId::FalloffMultiplier,
            "sway" | "sway_multiplier" => WeaponStatId::SwayMultiplier,
            "ads_fov" | "ads_fov_multiplier" => WeaponStatId::AdsFovMultiplier,
            "ads_sensitivity" | "ads_sensitivity_multiplier" => {
                WeaponStatId::AdsSensitivityMultiplier
            }
            "aim_in" | "aim_in_duration" | "aim_in_duration_multiplier" => {
                WeaponStatId::AimInDurationMultiplier
            }
            "aim_out" | "aim_out_duration" | "aim_out_duration_multiplier" => {
                WeaponStatId::AimOutDurationMultiplier
            }
            other => return Err(format!("unsupported weapon stat '{other}'")),
        };
        let operation = match self
            .operation
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "" | "multiply" | "mul" => WeaponStatModifierOp::Multiply,
            "add" | "additive" => WeaponStatModifierOp::Add,
            "override" | "set" => WeaponStatModifierOp::Override,
            "min" => WeaponStatModifierOp::Min,
            "max" => WeaponStatModifierOp::Max,
            other => return Err(format!("unsupported weapon stat operation '{other}'")),
        };
        if !self.value.is_finite() {
            return Err(format!(
                "weapon stat '{}' has non-finite modifier value",
                self.stat
            ));
        }
        Ok(WeaponStatModifier {
            stat,
            operation,
            value: self.value,
            priority: self.priority,
        })
    }
}

pub(super) fn compile_weapon_stat_stack(
    authored: &[AuthoredWeaponStatModifier],
) -> Result<WeaponStatModifierStack, String> {
    let modifiers = authored
        .iter()
        .map(AuthoredWeaponStatModifier::compile)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WeaponStatModifierStack { modifiers }.sanitized())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_authored_profiles_override_legacy_without_erasing_other_values() {
        let legacy = HitscanWeaponTuning {
            reload_duration: 1.9,
            hip_spread_radians: 1.4_f32.to_radians(),
            recoil_pitch_radians: 0.9_f32.to_radians(),
            ..HitscanWeaponTuning::default()
        };
        let authored = AuthoredWeaponRuntimeProfiles {
            handling: AuthoredWeaponHandlingProfile {
                aim_in_duration_seconds: Some(0.11),
                ..Default::default()
            },
            spread: AuthoredWeaponSpreadProfile {
                distribution: Some("circular".to_owned()),
                hip: AuthoredWeaponSpreadStateProfile {
                    default_degrees: Some([1.2, 1.7]),
                    ..Default::default()
                },
                ..Default::default()
            },
            recoil: AuthoredWeaponRecoilProfile {
                ads: AuthoredWeaponRecoilStateProfile {
                    hold_seconds: Some(0.045),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let compiled = authored.compile(legacy).expect("profiles compile");
        assert!((compiled.handling.reload_duration_seconds - 1.9).abs() < 1.0e-6);
        assert!((compiled.handling.aim_in_duration_seconds - 0.11).abs() < 1.0e-6);
        assert_eq!(
            compiled.spread.distribution,
            WeaponSpreadDistribution::Circular
        );
        assert!((compiled.spread.hip.default_radians[0].to_degrees() - 1.2).abs() < 1.0e-5);
        assert!((compiled.recoil.ads.hold_seconds - 0.045).abs() < 1.0e-6);
        assert!((compiled.recoil.hip.pitch_radians - legacy.recoil_pitch_radians).abs() < 1.0e-6);
    }
}
