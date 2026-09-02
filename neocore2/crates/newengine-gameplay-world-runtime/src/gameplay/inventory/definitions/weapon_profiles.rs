#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponSpreadDistribution {
    #[default]
    Rectangular,
    Circular,
    Gaussian,
    EvenJitter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponReloadTopology {
    #[default]
    DetachableMagazine,
    InternalMagazine,
    SingleRound,
}

impl WeaponReloadTopology {
    #[inline]
    pub const fn required_animation_marker_mask(self) -> u8 {
        const MAG_DETACH: u8 = 1 << 0;
        const AMMO_COMMIT: u8 = 1 << 1;
        const MAG_INSERT: u8 = 1 << 2;
        const CHAMBER: u8 = 1 << 3;
        const COMPLETE: u8 = 1 << 4;
        match self {
            Self::DetachableMagazine => MAG_DETACH | AMMO_COMMIT | MAG_INSERT | CHAMBER | COMPLETE,
            Self::InternalMagazine | Self::SingleRound => AMMO_COMMIT | CHAMBER | COMPLETE,
        }
    }

    #[inline]
    pub const fn uses_detachable_magazine(self) -> bool {
        matches!(self, Self::DetachableMagazine)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponReloadTimelineProfile {
    pub magazine_detach_fraction: f32,
    pub ammo_commit_fraction: f32,
    pub magazine_insert_fraction: f32,
    pub chamber_fraction: f32,
}

impl WeaponReloadTimelineProfile {
    pub fn sanitized(self) -> Self {
        let detach = finite_or(self.magazine_detach_fraction, 0.30).clamp(0.0, 1.0);
        let commit = finite_or(self.ammo_commit_fraction, 0.65).clamp(detach, 1.0);
        let insert = finite_or(self.magazine_insert_fraction, 0.70).clamp(commit, 1.0);
        let chamber = finite_or(self.chamber_fraction, 0.90).clamp(insert, 1.0);
        Self {
            magazine_detach_fraction: detach,
            ammo_commit_fraction: commit,
            magazine_insert_fraction: insert,
            chamber_fraction: chamber,
        }
    }
}

impl Default for WeaponReloadTimelineProfile {
    fn default() -> Self {
        Self {
            magazine_detach_fraction: 0.30,
            ammo_commit_fraction: 0.65,
            magazine_insert_fraction: 0.70,
            chamber_fraction: 0.90,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponHandlingProfile {
    pub reload_duration_seconds: f32,
    pub reload_topology: WeaponReloadTopology,
    pub equip_duration_seconds: f32,
    pub unequip_duration_seconds: f32,
    pub aim_in_duration_seconds: f32,
    pub aim_out_duration_seconds: f32,
    pub muzzle_forward_offset: f32,
    pub reload_timeline: WeaponReloadTimelineProfile,
}

impl WeaponHandlingProfile {
    pub fn sanitized(self) -> Self {
        Self {
            reload_duration_seconds: finite_or(self.reload_duration_seconds, 1.8).clamp(0.0, 120.0),
            reload_topology: self.reload_topology,
            equip_duration_seconds: finite_or(self.equip_duration_seconds, 0.3).clamp(0.0, 30.0),
            unequip_duration_seconds: finite_or(self.unequip_duration_seconds, 0.25)
                .clamp(0.0, 30.0),
            aim_in_duration_seconds: finite_or(self.aim_in_duration_seconds, 0.15).clamp(0.0, 10.0),
            aim_out_duration_seconds: finite_or(self.aim_out_duration_seconds, 0.2)
                .clamp(0.0, 10.0),
            muzzle_forward_offset: finite_or(self.muzzle_forward_offset, 0.52).clamp(0.0, 10.0),
            reload_timeline: self.reload_timeline.sanitized(),
        }
    }
}

impl Default for WeaponHandlingProfile {
    fn default() -> Self {
        Self {
            reload_duration_seconds: 1.8,
            reload_topology: WeaponReloadTopology::DetachableMagazine,
            equip_duration_seconds: 0.3,
            unequip_duration_seconds: 0.25,
            aim_in_duration_seconds: 0.15,
            aim_out_duration_seconds: 0.2,
            muzzle_forward_offset: 0.52,
            reload_timeline: WeaponReloadTimelineProfile::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSpreadStateProfile {
    pub default_radians: [f32; 2],
    pub minimum_radians: [f32; 2],
    pub maximum_radians: [f32; 2],
    pub change_per_shot_radians: [f32; 2],
}

impl WeaponSpreadStateProfile {
    pub fn sanitized(self) -> Self {
        let sanitize_angle =
            |value: f32| finite_or(value, 0.0).clamp(0.0, core::f32::consts::FRAC_PI_2 - 0.001);
        let minimum = self.minimum_radians.map(sanitize_angle);
        let maximum_raw = self.maximum_radians.map(sanitize_angle);
        let maximum = [
            maximum_raw[0].max(minimum[0]),
            maximum_raw[1].max(minimum[1]),
        ];
        let default_raw = self.default_radians.map(sanitize_angle);
        Self {
            default_radians: [
                default_raw[0].clamp(minimum[0], maximum[0]),
                default_raw[1].clamp(minimum[1], maximum[1]),
            ],
            minimum_radians: minimum,
            maximum_radians: maximum,
            change_per_shot_radians: self.change_per_shot_radians.map(sanitize_angle),
        }
    }

    #[inline]
    pub fn scalar_default(self) -> f32 {
        let value = self.sanitized().default_radians;
        value[0].max(value[1])
    }
}

impl Default for WeaponSpreadStateProfile {
    fn default() -> Self {
        Self {
            default_radians: [1.5_f32.to_radians(); 2],
            minimum_radians: [0.0; 2],
            maximum_radians: [8.0_f32.to_radians(); 2],
            change_per_shot_radians: [0.28_f32.to_radians(); 2],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSpreadProfile {
    pub hip: WeaponSpreadStateProfile,
    pub ads: WeaponSpreadStateProfile,
    pub distribution: WeaponSpreadDistribution,
    pub movement_multiplier: f32,
    pub crouch_multiplier: f32,
    pub recovery_hz: f32,
    pub recovery_delay_seconds: f32,
    pub zero_first_shot: bool,
}

impl WeaponSpreadProfile {
    pub fn sanitized(self) -> Self {
        Self {
            hip: self.hip.sanitized(),
            ads: self.ads.sanitized(),
            distribution: self.distribution,
            movement_multiplier: finite_or(self.movement_multiplier, 1.65).clamp(1.0, 8.0),
            crouch_multiplier: finite_or(self.crouch_multiplier, 0.82).clamp(0.1, 2.0),
            recovery_hz: finite_or(self.recovery_hz, 5.5).clamp(0.05, 120.0),
            recovery_delay_seconds: finite_or(self.recovery_delay_seconds, 0.1).clamp(0.0, 5.0),
            zero_first_shot: self.zero_first_shot,
        }
    }
}

impl Default for WeaponSpreadProfile {
    fn default() -> Self {
        let hip = WeaponSpreadStateProfile::default();
        Self {
            hip,
            ads: WeaponSpreadStateProfile {
                default_radians: [0.25_f32.to_radians(); 2],
                minimum_radians: [0.0; 2],
                maximum_radians: [3.5_f32.to_radians(); 2],
                change_per_shot_radians: [0.18_f32.to_radians(); 2],
            },
            distribution: WeaponSpreadDistribution::Rectangular,
            movement_multiplier: 1.65,
            crouch_multiplier: 0.82,
            recovery_hz: 5.5,
            recovery_delay_seconds: 0.1,
            zero_first_shot: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRecoilStateProfile {
    pub pitch_radians: f32,
    pub pitch_random_radians: f32,
    pub yaw_radians: f32,
    pub yaw_bias_radians: f32,
    pub recovery_hz: f32,
    pub hold_seconds: f32,
    pub max_accumulated_radians: f32,
    pub pitch_tracker_speed_scale: f32,
    pub yaw_tracker_speed_scale: f32,
}

impl WeaponRecoilStateProfile {
    pub fn sanitized(self) -> Self {
        Self {
            pitch_radians: finite_or(self.pitch_radians, 0.0).clamp(0.0, 1.0),
            pitch_random_radians: finite_or(self.pitch_random_radians, 0.0).clamp(0.0, 1.0),
            yaw_radians: finite_or(self.yaw_radians, 0.0).clamp(0.0, 1.0),
            yaw_bias_radians: finite_or(self.yaw_bias_radians, 0.0).clamp(-1.0, 1.0),
            recovery_hz: finite_or(self.recovery_hz, 7.5).clamp(0.05, 120.0),
            hold_seconds: finite_or(self.hold_seconds, 0.0).clamp(0.0, 5.0),
            max_accumulated_radians: finite_or(self.max_accumulated_radians, 30_f32.to_radians())
                .clamp(0.0, core::f32::consts::PI),
            pitch_tracker_speed_scale: finite_or(self.pitch_tracker_speed_scale, 1.4)
                .clamp(0.0, 4.0),
            yaw_tracker_speed_scale: finite_or(self.yaw_tracker_speed_scale, 1.15).clamp(0.0, 4.0),
        }
    }
}

impl Default for WeaponRecoilStateProfile {
    fn default() -> Self {
        Self {
            pitch_radians: 0.8_f32.to_radians(),
            pitch_random_radians: 0.15_f32.to_radians(),
            yaw_radians: 0.35_f32.to_radians(),
            yaw_bias_radians: 0.0,
            recovery_hz: 7.5,
            hold_seconds: 0.0,
            max_accumulated_radians: 30_f32.to_radians(),
            pitch_tracker_speed_scale: 1.4,
            yaw_tracker_speed_scale: 1.15,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRecoilProfile {
    pub hip: WeaponRecoilStateProfile,
    pub ads: WeaponRecoilStateProfile,
    pub drift_min_radians: f32,
    pub drift_max_radians: f32,
    pub drift_full_kick_radians: f32,
}

impl WeaponRecoilProfile {
    pub fn sanitized(self) -> Self {
        let drift_min = finite_or(self.drift_min_radians, 0.0).clamp(0.0, 1.0);
        Self {
            hip: self.hip.sanitized(),
            ads: self.ads.sanitized(),
            drift_min_radians: drift_min,
            drift_max_radians: finite_or(self.drift_max_radians, drift_min).clamp(drift_min, 1.0),
            drift_full_kick_radians: finite_or(self.drift_full_kick_radians, 3_f32.to_radians())
                .clamp(0.001, 1.0),
        }
    }

    #[inline]
    pub fn state(self, aiming: bool) -> WeaponRecoilStateProfile {
        let value = self.sanitized();
        if aiming {
            value.ads
        } else {
            value.hip
        }
    }
}

impl Default for WeaponRecoilProfile {
    fn default() -> Self {
        let hip = WeaponRecoilStateProfile::default();
        let mut ads = hip;
        ads.pitch_radians *= 0.78;
        ads.pitch_random_radians *= 0.78;
        ads.yaw_radians *= 0.78;
        ads.yaw_bias_radians *= 0.78;
        Self {
            hip,
            ads,
            drift_min_radians: 0.2_f32.to_radians(),
            drift_max_radians: 0.5_f32.to_radians(),
            drift_full_kick_radians: 3_f32.to_radians(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSwayProfile {
    pub traversal_seconds: f32,
    pub horizontal_radians: f32,
    pub vertical_radians: f32,
    pub start_delay_seconds: f32,
    pub blend_in_seconds: f32,
    pub ads_multiplier: f32,
}

impl WeaponSwayProfile {
    pub fn sanitized(self) -> Self {
        Self {
            traversal_seconds: finite_or(self.traversal_seconds, 6.0).clamp(0.05, 60.0),
            horizontal_radians: finite_or(self.horizontal_radians, 1.8_f32.to_radians())
                .clamp(0.0, 1.0),
            vertical_radians: finite_or(self.vertical_radians, 1.6_f32.to_radians())
                .clamp(0.0, 1.0),
            start_delay_seconds: finite_or(self.start_delay_seconds, 0.03).clamp(0.0, 10.0),
            blend_in_seconds: finite_or(self.blend_in_seconds, 2.0).clamp(0.0, 30.0),
            ads_multiplier: finite_or(self.ads_multiplier, 0.35).clamp(0.0, 4.0),
        }
    }
}

impl Default for WeaponSwayProfile {
    fn default() -> Self {
        Self {
            traversal_seconds: 6.0,
            horizontal_radians: 1.8_f32.to_radians(),
            vertical_radians: 1.6_f32.to_radians(),
            start_delay_seconds: 0.03,
            blend_in_seconds: 2.0,
            ads_multiplier: 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponAdsProfile {
    pub recoil_multiplier: f32,
    pub fov_multiplier: f32,
    pub sensitivity_multiplier: f32,
}

impl WeaponAdsProfile {
    pub fn sanitized(self) -> Self {
        Self {
            recoil_multiplier: finite_or(self.recoil_multiplier, 0.78).clamp(0.0, 4.0),
            fov_multiplier: finite_or(self.fov_multiplier, 0.72).clamp(0.1, 1.5),
            sensitivity_multiplier: finite_or(self.sensitivity_multiplier, 0.8).clamp(0.05, 2.0),
        }
    }
}

impl Default for WeaponAdsProfile {
    fn default() -> Self {
        Self {
            recoil_multiplier: 0.78,
            fov_multiplier: 0.72,
            sensitivity_multiplier: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRuntimeProfiles {
    pub handling: WeaponHandlingProfile,
    pub spread: WeaponSpreadProfile,
    pub recoil: WeaponRecoilProfile,
    pub sway: WeaponSwayProfile,
    pub ads: WeaponAdsProfile,
}

impl WeaponRuntimeProfiles {
    pub fn from_legacy_tuning(tuning: HitscanWeaponTuning) -> Self {
        let tuning = tuning.sanitized();
        let hip_spread = WeaponSpreadStateProfile {
            default_radians: [tuning.hip_spread_radians; 2],
            minimum_radians: [0.0; 2],
            maximum_radians: [tuning
                .recoil_accuracy_max_radians
                .max(tuning.hip_spread_radians); 2],
            change_per_shot_radians: [tuning.recoil_accuracy_per_shot_radians; 2],
        };
        let ads_spread = WeaponSpreadStateProfile {
            default_radians: [tuning.aim_spread_radians; 2],
            minimum_radians: [0.0; 2],
            maximum_radians: [tuning
                .recoil_accuracy_max_radians
                .max(tuning.aim_spread_radians); 2],
            change_per_shot_radians: [tuning.recoil_accuracy_per_shot_radians; 2],
        };
        let hip_recoil = WeaponRecoilStateProfile {
            pitch_radians: tuning.recoil_pitch_radians,
            pitch_random_radians: tuning.recoil_pitch_random_radians,
            yaw_radians: tuning.recoil_yaw_radians,
            yaw_bias_radians: tuning.recoil_yaw_bias_radians,
            recovery_hz: tuning.recoil_recovery_hz,
            hold_seconds: 0.0,
            max_accumulated_radians: 30_f32.to_radians(),
            pitch_tracker_speed_scale: tuning.recoil_pitch_tracker_speed_scale,
            yaw_tracker_speed_scale: tuning.recoil_yaw_tracker_speed_scale,
        };
        let mut ads_recoil = hip_recoil;
        ads_recoil.pitch_radians *= tuning.ads_recoil_multiplier;
        ads_recoil.pitch_random_radians *= tuning.ads_recoil_multiplier;
        ads_recoil.yaw_radians *= tuning.ads_recoil_multiplier;
        ads_recoil.yaw_bias_radians *= tuning.ads_recoil_multiplier;
        Self {
            handling: WeaponHandlingProfile {
                reload_duration_seconds: tuning.reload_duration,
                muzzle_forward_offset: tuning.muzzle_forward_offset,
                ..WeaponHandlingProfile::default()
            },
            spread: WeaponSpreadProfile {
                hip: hip_spread,
                ads: ads_spread,
                distribution: WeaponSpreadDistribution::Rectangular,
                movement_multiplier: tuning.movement_spread_multiplier,
                crouch_multiplier: tuning.crouch_spread_multiplier,
                recovery_hz: tuning.accuracy_recovery_hz,
                recovery_delay_seconds: tuning.accuracy_recovery_delay_seconds,
                zero_first_shot: false,
            },
            recoil: WeaponRecoilProfile {
                hip: hip_recoil,
                ads: ads_recoil,
                ..WeaponRecoilProfile::default()
            },
            sway: WeaponSwayProfile::default(),
            ads: WeaponAdsProfile {
                recoil_multiplier: tuning.ads_recoil_multiplier,
                ..WeaponAdsProfile::default()
            },
        }
        .sanitized()
    }

    pub fn sanitized(self) -> Self {
        Self {
            handling: self.handling.sanitized(),
            spread: self.spread.sanitized(),
            recoil: self.recoil.sanitized(),
            sway: self.sway.sanitized(),
            ads: self.ads.sanitized(),
        }
    }
}

impl Default for WeaponRuntimeProfiles {
    fn default() -> Self {
        Self::from_legacy_tuning(HitscanWeaponTuning::default())
    }
}

#[cfg(test)]
mod weapon_profile_tests {
    use super::*;

    #[test]
    fn legacy_projection_preserves_core_weapon_feel() {
        let tuning = HitscanWeaponTuning {
            reload_duration: 2.25,
            hip_spread_radians: 2.0_f32.to_radians(),
            aim_spread_radians: 0.3_f32.to_radians(),
            recoil_pitch_radians: 1.1_f32.to_radians(),
            recoil_yaw_radians: 0.45_f32.to_radians(),
            ads_recoil_multiplier: 0.6,
            ..HitscanWeaponTuning::default()
        };
        let profiles = WeaponRuntimeProfiles::from_legacy_tuning(tuning);
        assert!((profiles.handling.reload_duration_seconds - 2.25).abs() < 1.0e-6);
        assert!((profiles.spread.hip.scalar_default() - tuning.hip_spread_radians).abs() < 1.0e-6);
        assert!((profiles.spread.ads.scalar_default() - tuning.aim_spread_radians).abs() < 1.0e-6);
        assert!((profiles.recoil.hip.pitch_radians - tuning.recoil_pitch_radians).abs() < 1.0e-6);
        assert!(
            (profiles.recoil.ads.pitch_radians - tuning.recoil_pitch_radians * 0.6).abs() < 1.0e-6
        );
    }

    #[test]
    fn profile_sanitization_keeps_spread_bounds_coherent() {
        let profile = WeaponSpreadStateProfile {
            default_radians: [4.0, -1.0],
            minimum_radians: [0.5, 0.25],
            maximum_radians: [0.2, 0.1],
            change_per_shot_radians: [-2.0, 99.0],
        }
        .sanitized();
        assert!(profile.maximum_radians[0] >= profile.minimum_radians[0]);
        assert!(profile.maximum_radians[1] >= profile.minimum_radians[1]);
        assert!(profile.default_radians[0] >= profile.minimum_radians[0]);
        assert!(profile.change_per_shot_radians[0] >= 0.0);
    }
}
