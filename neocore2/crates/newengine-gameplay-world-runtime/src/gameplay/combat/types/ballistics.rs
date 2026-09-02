#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponAttackKind {
    Melee,
    Firearm,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallisticShotProfile {
    pub projectile_mass_kg: f32,
    pub muzzle_velocity_mps: f32,
    pub momentum_ns: f32,
    pub remaining_penetration_energy_j: f32,
    pub max_penetration_m: f32,
    pub damage_multiplier: f32,
    pub impulse_multiplier: f32,
    pub falloff_start_m: f32,
    pub falloff_end_m: f32,
    pub falloff_min_multiplier: f32,
    pub component_falloff_multiplier: f32,
}

impl BallisticShotProfile {
    pub fn sanitized(self) -> Self {
        let mass = if self.projectile_mass_kg.is_finite() {
            self.projectile_mass_kg.clamp(0.0001, 1.0)
        } else {
            0.008
        };
        let velocity = if self.muzzle_velocity_mps.is_finite() {
            self.muzzle_velocity_mps.clamp(1.0, 2_500.0)
        } else {
            350.0
        };
        Self {
            projectile_mass_kg: mass,
            muzzle_velocity_mps: velocity,
            momentum_ns: if self.momentum_ns.is_finite() {
                self.momentum_ns.max(0.0)
            } else {
                mass * velocity
            },
            remaining_penetration_energy_j: if self.remaining_penetration_energy_j.is_finite() {
                self.remaining_penetration_energy_j.max(0.0)
            } else {
                0.0
            },
            max_penetration_m: if self.max_penetration_m.is_finite() {
                self.max_penetration_m.clamp(0.0, 10.0)
            } else {
                0.0
            },
            damage_multiplier: if self.damage_multiplier.is_finite() {
                self.damage_multiplier.clamp(0.0, 20.0)
            } else {
                1.0
            },
            impulse_multiplier: if self.impulse_multiplier.is_finite() {
                self.impulse_multiplier.clamp(0.0, 20.0)
            } else {
                1.0
            },
            falloff_start_m: if self.falloff_start_m.is_finite() {
                self.falloff_start_m.clamp(0.0, 10_000.0)
            } else {
                0.0
            },
            falloff_end_m: if self.falloff_end_m.is_finite() {
                self.falloff_end_m.clamp(0.001, 10_000.0)
            } else {
                100.0
            },
            falloff_min_multiplier: if self.falloff_min_multiplier.is_finite() {
                self.falloff_min_multiplier.clamp(0.0, 1.0)
            } else {
                1.0
            },
            component_falloff_multiplier: if self.component_falloff_multiplier.is_finite() {
                self.component_falloff_multiplier.clamp(0.0, 20.0)
            } else {
                1.0
            },
        }
    }
}

impl BallisticShotProfile {
    pub fn falloff_multiplier_at(self, distance_m: f32) -> f32 {
        let value = self.sanitized();
        let end = value.falloff_end_m.max(value.falloff_start_m + 0.001);
        let curve = if distance_m <= value.falloff_start_m {
            1.0
        } else if distance_m >= end {
            value.falloff_min_multiplier
        } else {
            let alpha = ((distance_m - value.falloff_start_m) / (end - value.falloff_start_m))
                .clamp(0.0, 1.0);
            1.0 + (value.falloff_min_multiplier - 1.0) * alpha
        };
        curve * value.component_falloff_multiplier
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingHitscan {
    pub query_seq: u64,
    /// Weapon identity is captured at trigger time and survives equipment switches while the
    /// asynchronous physics query is in flight.
    pub weapon_instance_id: ItemInstanceId,
    pub attack_kind: WeaponAttackKind,
    pub shot_sequence: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub damage: f32,
    pub ballistics: BallisticShotProfile,
    pub bounce_count: u8,
    pub max_bounces: u8,
    pub ricochet_grazing_dot: f32,
    pub ricochet_energy_retention: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingInteraction {
    pub query_seq: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
}
