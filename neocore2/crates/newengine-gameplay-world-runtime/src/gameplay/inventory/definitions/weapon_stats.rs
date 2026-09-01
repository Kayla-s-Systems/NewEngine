#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponStatId {
    DamageMultiplier,
    RecoilMultiplier,
    SpreadMultiplier,
    ReloadDurationMultiplier,
    MuzzleVelocityMultiplier,
    PenetrationMultiplier,
    FalloffMultiplier,
    SwayMultiplier,
    AdsFovMultiplier,
    AdsSensitivityMultiplier,
    AimInDurationMultiplier,
    AimOutDurationMultiplier,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponStatModifierOp {
    Add,
    #[default]
    Multiply,
    Override,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponStatModifier {
    pub stat: WeaponStatId,
    pub operation: WeaponStatModifierOp,
    pub value: f32,
    /// Lower priorities resolve first. Equal-priority modifiers preserve authored sequence.
    pub priority: i16,
}

impl WeaponStatModifier {
    #[inline]
    pub const fn multiply(stat: WeaponStatId, value: f32) -> Self {
        Self {
            stat,
            operation: WeaponStatModifierOp::Multiply,
            value,
            priority: 0,
        }
    }

    #[inline]
    pub const fn additive(stat: WeaponStatId, value: f32) -> Self {
        Self {
            stat,
            operation: WeaponStatModifierOp::Add,
            value,
            priority: 0,
        }
    }

    #[inline]
    pub const fn with_priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponStatModifierStack {
    pub modifiers: Vec<WeaponStatModifier>,
}

impl WeaponStatModifierStack {
    pub fn sanitized(mut self) -> Self {
        self.modifiers.retain(|modifier| modifier.value.is_finite());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedWeaponStats {
    pub damage_multiplier: f32,
    pub recoil_multiplier: f32,
    pub spread_multiplier: f32,
    pub reload_duration_multiplier: f32,
    pub muzzle_velocity_multiplier: f32,
    pub penetration_multiplier: f32,
    pub falloff_multiplier: f32,
    pub sway_multiplier: f32,
    pub ads_fov_multiplier: f32,
    pub ads_sensitivity_multiplier: f32,
    pub aim_in_duration_multiplier: f32,
    pub aim_out_duration_multiplier: f32,
}

impl Default for ResolvedWeaponStats {
    fn default() -> Self {
        Self {
            damage_multiplier: 1.0,
            recoil_multiplier: 1.0,
            spread_multiplier: 1.0,
            reload_duration_multiplier: 1.0,
            muzzle_velocity_multiplier: 1.0,
            penetration_multiplier: 1.0,
            falloff_multiplier: 1.0,
            sway_multiplier: 1.0,
            ads_fov_multiplier: 1.0,
            ads_sensitivity_multiplier: 1.0,
            aim_in_duration_multiplier: 1.0,
            aim_out_duration_multiplier: 1.0,
        }
    }
}

impl ResolvedWeaponStats {
    pub fn from_component_modifiers(modifiers: WeaponComponentModifiers) -> Self {
        let modifiers = modifiers.sanitized();
        Self {
            damage_multiplier: modifiers.damage_multiplier,
            recoil_multiplier: modifiers.recoil_multiplier,
            spread_multiplier: modifiers.accuracy_multiplier,
            muzzle_velocity_multiplier: modifiers.muzzle_velocity_multiplier,
            penetration_multiplier: modifiers.penetration_multiplier,
            falloff_multiplier: modifiers.falloff_multiplier,
            ..Self::default()
        }
        .sanitized()
    }

    pub fn resolve_stacks<'a>(
        base: Self,
        stacks: impl IntoIterator<Item = &'a WeaponStatModifierStack>,
    ) -> Self {
        let mut resolved = base.sanitized();
        let mut ordered = stacks
            .into_iter()
            .flat_map(|stack| stack.modifiers.iter().copied().enumerate())
            .enumerate()
            .map(|(stack_order, (authored_order, modifier))| {
                (modifier.priority, stack_order, authored_order, modifier)
            })
            .filter(|(_, _, _, modifier)| modifier.value.is_finite())
            .collect::<Vec<_>>();
        ordered.sort_by_key(|(priority, stack_order, authored_order, _)| {
            (*priority, *stack_order, *authored_order)
        });
        for (_, _, _, modifier) in ordered {
            resolved.apply(modifier);
        }
        resolved.sanitized()
    }

    pub fn apply(&mut self, modifier: WeaponStatModifier) {
        let target = match modifier.stat {
            WeaponStatId::DamageMultiplier => &mut self.damage_multiplier,
            WeaponStatId::RecoilMultiplier => &mut self.recoil_multiplier,
            WeaponStatId::SpreadMultiplier => &mut self.spread_multiplier,
            WeaponStatId::ReloadDurationMultiplier => &mut self.reload_duration_multiplier,
            WeaponStatId::MuzzleVelocityMultiplier => &mut self.muzzle_velocity_multiplier,
            WeaponStatId::PenetrationMultiplier => &mut self.penetration_multiplier,
            WeaponStatId::FalloffMultiplier => &mut self.falloff_multiplier,
            WeaponStatId::SwayMultiplier => &mut self.sway_multiplier,
            WeaponStatId::AdsFovMultiplier => &mut self.ads_fov_multiplier,
            WeaponStatId::AdsSensitivityMultiplier => &mut self.ads_sensitivity_multiplier,
            WeaponStatId::AimInDurationMultiplier => &mut self.aim_in_duration_multiplier,
            WeaponStatId::AimOutDurationMultiplier => &mut self.aim_out_duration_multiplier,
        };
        *target = apply_stat_operation(*target, modifier.operation, modifier.value);
    }

    pub fn sanitized(self) -> Self {
        let positive = |value: f32, default: f32| {
            if value.is_finite() {
                value.clamp(0.0, 20.0)
            } else {
                default
            }
        };
        Self {
            damage_multiplier: positive(self.damage_multiplier, 1.0),
            recoil_multiplier: positive(self.recoil_multiplier, 1.0),
            spread_multiplier: positive(self.spread_multiplier, 1.0),
            reload_duration_multiplier: positive(self.reload_duration_multiplier, 1.0),
            muzzle_velocity_multiplier: positive(self.muzzle_velocity_multiplier, 1.0),
            penetration_multiplier: positive(self.penetration_multiplier, 1.0),
            falloff_multiplier: positive(self.falloff_multiplier, 1.0),
            sway_multiplier: positive(self.sway_multiplier, 1.0),
            ads_fov_multiplier: positive(self.ads_fov_multiplier, 1.0),
            ads_sensitivity_multiplier: positive(self.ads_sensitivity_multiplier, 1.0),
            aim_in_duration_multiplier: positive(self.aim_in_duration_multiplier, 1.0),
            aim_out_duration_multiplier: positive(self.aim_out_duration_multiplier, 1.0),
        }
    }
}

#[inline]
fn apply_stat_operation(current: f32, operation: WeaponStatModifierOp, value: f32) -> f32 {
    if !current.is_finite() || !value.is_finite() {
        return current;
    }
    match operation {
        WeaponStatModifierOp::Add => current + value,
        WeaponStatModifierOp::Multiply => current * value,
        WeaponStatModifierOp::Override => value,
        WeaponStatModifierOp::Min => current.min(value),
        WeaponStatModifierOp::Max => current.max(value),
    }
}

#[cfg(test)]
mod weapon_stat_tests {
    use super::*;

    #[test]
    fn modifier_stack_is_priority_ordered_and_deterministic() {
        let low = WeaponStatModifierStack {
            modifiers: vec![
                WeaponStatModifier::multiply(WeaponStatId::RecoilMultiplier, 0.8).with_priority(0),
                WeaponStatModifier {
                    stat: WeaponStatId::RecoilMultiplier,
                    operation: WeaponStatModifierOp::Add,
                    value: 0.1,
                    priority: 10,
                },
            ],
        };
        let high = WeaponStatModifierStack {
            modifiers: vec![WeaponStatModifier {
                stat: WeaponStatId::RecoilMultiplier,
                operation: WeaponStatModifierOp::Override,
                value: 0.5,
                priority: 20,
            }],
        };
        let resolved =
            ResolvedWeaponStats::resolve_stacks(ResolvedWeaponStats::default(), [&low, &high]);
        assert!((resolved.recoil_multiplier - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn typed_component_modifiers_project_into_generic_stat_space() {
        let resolved = ResolvedWeaponStats::from_component_modifiers(WeaponComponentModifiers {
            recoil_multiplier: 0.75,
            accuracy_multiplier: 0.8,
            damage_multiplier: 1.1,
            ..WeaponComponentModifiers::default()
        });
        assert!((resolved.recoil_multiplier - 0.75).abs() < 1.0e-6);
        assert!((resolved.spread_multiplier - 0.8).abs() < 1.0e-6);
        assert!((resolved.damage_multiplier - 1.1).abs() < 1.0e-6);
    }
}
