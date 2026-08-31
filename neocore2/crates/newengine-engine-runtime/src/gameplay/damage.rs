use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;

use super::Health;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DamageReceiverKind {
    Character,
    Vehicle,
    Destructible,
    Glass,
    #[default]
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageReceiver {
    pub kind: DamageReceiverKind,
    pub damage_multiplier: f32,
    /// Fraction of incoming damage absorbed before hit-zone modifiers, in `[0, 0.98]`.
    pub armor_absorption: f32,
    pub impulse_multiplier: f32,
}

impl DamageReceiver {
    #[inline]
    pub const fn character() -> Self {
        Self {
            kind: DamageReceiverKind::Character,
            damage_multiplier: 1.0,
            armor_absorption: 0.0,
            impulse_multiplier: 1.0,
        }
    }

    #[inline]
    pub const fn generic() -> Self {
        Self {
            kind: DamageReceiverKind::Generic,
            damage_multiplier: 1.0,
            armor_absorption: 0.0,
            impulse_multiplier: 1.0,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            kind: self.kind,
            damage_multiplier: finite_or(self.damage_multiplier, 1.0).clamp(0.0, 20.0),
            armor_absorption: finite_or(self.armor_absorption, 0.0).clamp(0.0, 0.98),
            impulse_multiplier: finite_or(self.impulse_multiplier, 1.0).clamp(0.0, 20.0),
        }
    }
}

impl Default for DamageReceiver {
    fn default() -> Self {
        Self::generic()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageHitZone {
    pub id: String,
    pub damage_multiplier: f32,
    pub armor_absorption: f32,
    pub impulse_multiplier: f32,
}

impl DamageHitZone {
    pub fn sanitized(mut self) -> Self {
        self.id = self.id.trim().to_ascii_lowercase();
        self.damage_multiplier = finite_or(self.damage_multiplier, 1.0).clamp(0.0, 20.0);
        self.armor_absorption = finite_or(self.armor_absorption, 0.0).clamp(0.0, 0.98);
        self.impulse_multiplier = finite_or(self.impulse_multiplier, 1.0).clamp(0.0, 20.0);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageHitZoneMap {
    pub by_subshape: BTreeMap<u32, DamageHitZone>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallisticMaterialResponse {
    /// Energy consumed per metre of real traversed geometry.
    pub penetration_resistance_j_per_m: f32,
    /// Fixed surface-break cost paid once at entry.
    pub entry_energy_cost_j: f32,
    pub damage_transfer_multiplier: f32,
    pub impulse_transfer_multiplier: f32,
    /// Material-owned ricochet policy. Gameplay must not infer this from surface-name strings.
    pub ricochet_allowed: bool,
    /// Maximum absolute incidence dot (0=grazing, 1=head-on) that may ricochet.
    pub ricochet_max_incidence_dot: f32,
    pub ricochet_energy_retention: f32,
}

impl BallisticMaterialResponse {
    pub fn sanitized(self) -> Self {
        Self {
            penetration_resistance_j_per_m: finite_or(
                self.penetration_resistance_j_per_m,
                f32::INFINITY,
            )
            .max(0.0),
            entry_energy_cost_j: finite_or(self.entry_energy_cost_j, f32::INFINITY).max(0.0),
            damage_transfer_multiplier: finite_or(self.damage_transfer_multiplier, 1.0)
                .clamp(0.0, 20.0),
            impulse_transfer_multiplier: finite_or(self.impulse_transfer_multiplier, 1.0)
                .clamp(0.0, 20.0),
            ricochet_allowed: self.ricochet_allowed,
            ricochet_max_incidence_dot: finite_or(self.ricochet_max_incidence_dot, 0.0)
                .clamp(0.0, 1.0),
            ricochet_energy_retention: finite_or(self.ricochet_energy_retention, 0.0)
                .clamp(0.0, 1.0),
        }
    }

    #[inline]
    pub fn penetration_cost_j(self, thickness_m: f32) -> f32 {
        let value = self.sanitized();
        value.entry_energy_cost_j + value.penetration_resistance_j_per_m * thickness_m.max(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponImpact {
    pub sequence: u64,
    pub source: EntityId,
    pub target: EntityId,
    pub base_damage: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub direction: Vec3,
    pub distance: f32,
    pub range: f32,
    pub subshape_id: u32,
    pub momentum_ns: f32,
    pub ammo_impulse_multiplier: f32,
    /// Fully authored weapon/ammo/component falloff multiplier. Damage domain does not invent curves.
    pub falloff_multiplier: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageResolution {
    pub receiver_kind: DamageReceiverKind,
    pub hit_zone: Option<String>,
    pub requested_damage: f32,
    pub applied_damage: f32,
    pub impulse: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingPhysicsImpulse {
    pub sequence: u64,
    pub impulse: Vec3,
    pub point: Vec3,
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

pub fn resolve_weapon_impact(world: &mut World, impact: WeaponImpact) -> Option<DamageResolution> {
    // Fail closed: only authored damage receivers participate in the damage domain.
    let receiver = world.get::<DamageReceiver>(impact.target).copied()?.sanitized();
    let zone = world
        .get::<DamageHitZoneMap>(impact.target)
        .and_then(|zones| zones.by_subshape.get(&impact.subshape_id))
        .cloned()
        .map(DamageHitZone::sanitized);

    let falloff = finite_or(impact.falloff_multiplier, 1.0).clamp(0.0, 20.0);
    let zone_damage = zone
        .as_ref()
        .map(|zone| zone.damage_multiplier)
        .unwrap_or(1.0);
    let armor = 1.0
        - (1.0 - receiver.armor_absorption)
            * (1.0
                - zone
                    .as_ref()
                    .map(|zone| zone.armor_absorption)
                    .unwrap_or(0.0));
    let requested_damage = (finite_or(impact.base_damage, 0.0).max(0.0)
        * receiver.damage_multiplier
        * zone_damage
        * falloff
        * (1.0 - armor))
        .max(0.0);
    let applied_damage = world
        .get_mut::<Health>(impact.target)
        .map(|health| health.apply_damage(requested_damage))
        .unwrap_or(0.0);

    let impulse_scale = receiver.impulse_multiplier
        * zone
            .as_ref()
            .map(|zone| zone.impulse_multiplier)
            .unwrap_or(1.0)
        * finite_or(impact.ammo_impulse_multiplier, 1.0).max(0.0);
    let impulse = impact.direction.normalize_or_zero()
        * finite_or(impact.momentum_ns, 0.0).max(0.0)
        * impulse_scale;
    if impulse.length_squared() > 1.0e-10 {
        let _ = world.insert(
            impact.target,
            PendingPhysicsImpulse {
                sequence: impact.sequence,
                impulse,
                point: impact.point,
            },
        );
    }

    Some(DamageResolution {
        receiver_kind: receiver.kind,
        hit_zone: zone.map(|zone| zone.id),
        requested_damage,
        applied_damage,
        impulse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_hit_zone_and_armor_are_resolved_outside_weapon_runtime() {
        let mut world = World::new();
        let source = world.spawn();
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(
            target,
            DamageReceiver {
                kind: DamageReceiverKind::Character,
                damage_multiplier: 1.0,
                armor_absorption: 0.20,
                impulse_multiplier: 1.0,
            },
        );
        let _ = world.insert(
            target,
            DamageHitZoneMap {
                by_subshape: [(
                    7,
                    DamageHitZone {
                        id: "head".to_owned(),
                        damage_multiplier: 2.0,
                        armor_absorption: 0.0,
                        impulse_multiplier: 1.25,
                    },
                )]
                .into_iter()
                .collect(),
            },
        );
        let resolution = resolve_weapon_impact(
            &mut world,
            WeaponImpact {
                sequence: 1,
                source,
                target,
                base_damage: 25.0,
                point: Vec3::ZERO,
                normal: Vec3::Y,
                direction: -Vec3::Z,
                distance: 0.0,
                range: 100.0,
                subshape_id: 7,
                momentum_ns: 3.0,
                ammo_impulse_multiplier: 1.0,
                falloff_multiplier: 1.0,
            },
        )
        .expect("authored receiver");
        assert_eq!(resolution.receiver_kind, DamageReceiverKind::Character);
        assert_eq!(resolution.hit_zone.as_deref(), Some("head"));
        assert!((resolution.applied_damage - 40.0).abs() < 1.0e-4);
        assert!(world.get::<PendingPhysicsImpulse>(target).is_some());
    }
    #[test]
    fn weapon_impact_without_authored_receiver_is_rejected() {
        let mut world = World::new();
        let source = world.spawn();
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let result = resolve_weapon_impact(
            &mut world,
            WeaponImpact {
                sequence: 2, source, target, base_damage: 20.0, point: Vec3::ZERO,
                normal: Vec3::Y, direction: -Vec3::Z, distance: 5.0, range: 100.0,
                subshape_id: 0, momentum_ns: 2.0, ammo_impulse_multiplier: 1.0,
                falloff_multiplier: 1.0,
            },
        );
        assert!(result.is_none());
        assert!((world.get::<Health>(target).unwrap().current - 100.0).abs() < 1.0e-6);
    }

}
