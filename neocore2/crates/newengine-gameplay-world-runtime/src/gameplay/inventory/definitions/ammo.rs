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
