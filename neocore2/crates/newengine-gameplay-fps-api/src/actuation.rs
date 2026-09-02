/// Authored firearm engagement policy for an AI-controlled FPS actor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsAiCombatTuning {
    pub fire_distance: f32,
    pub aim_tolerance_radians: f32,
}

impl FpsAiCombatTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            fire_distance: if self.fire_distance.is_finite() {
                self.fire_distance.clamp(0.1, 1_000.0)
            } else {
                20.0
            },
            aim_tolerance_radians: if self.aim_tolerance_radians.is_finite() {
                self.aim_tolerance_radians
                    .clamp(0.001, core::f32::consts::PI)
            } else {
                3.0_f32.to_radians()
            },
        }
    }
}

impl Default for FpsAiCombatTuning {
    fn default() -> Self {
        Self {
            fire_distance: 20.0,
            aim_tolerance_radians: 3.0_f32.to_radians(),
        }
    }
}

/// Deferred authored loadout request for a non-player FPS actor.
///
/// The request carries only the project-authored logical loadout name. `fps-content-runtime`
/// resolves and applies it after ItemCatalog/InventoryLoadoutCatalog become available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FpsActorLoadoutRequest {
    pub loadout: String,
}

impl FpsActorLoadoutRequest {
    #[inline]
    pub fn new(loadout: impl Into<String>) -> Self {
        Self {
            loadout: loadout.into(),
        }
    }
}

/// Authored body-relative physical weapon mount for controller-driven actors without a skeletal
/// hand/socket presentation. This is a muzzle presentation contract only; it never owns ballistics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsActorWeaponMountTuning {
    pub local_offset: [f32; 3],
    pub local_forward: [f32; 3],
}

impl FpsActorWeaponMountTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        let finite3 = |value: [f32; 3], fallback: [f32; 3]| {
            [
                if value[0].is_finite() {
                    value[0]
                } else {
                    fallback[0]
                },
                if value[1].is_finite() {
                    value[1]
                } else {
                    fallback[1]
                },
                if value[2].is_finite() {
                    value[2]
                } else {
                    fallback[2]
                },
            ]
        };
        Self {
            local_offset: finite3(self.local_offset, [0.20, 1.20, -0.45]),
            local_forward: finite3(self.local_forward, [0.0, 0.0, -1.0]),
        }
    }
}

impl Default for FpsActorWeaponMountTuning {
    fn default() -> Self {
        Self {
            local_offset: [0.20, 1.20, -0.45],
            local_forward: [0.0, 0.0, -1.0],
        }
    }
}
