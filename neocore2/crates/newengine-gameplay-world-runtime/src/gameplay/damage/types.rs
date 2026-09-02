use super::*;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterHitReactionKind {
    #[default]
    None,
    Flinch,
    Stagger,
}

impl CharacterHitReactionKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Flinch => "flinch",
            Self::Stagger => "stagger",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterDamageResponseTuning {
    /// Applied damage / maximum health needed to request a stagger instead of a flinch.
    pub stagger_damage_fraction: f32,
    /// World-space impulse magnitude that independently requests stagger.
    pub stagger_impulse_threshold: f32,
    pub flinch_duration_seconds: f32,
    pub stagger_duration_seconds: f32,
    /// Character enters the persistent injured semantic state at or below this health fraction.
    pub injured_health_fraction: f32,
}

impl CharacterDamageResponseTuning {
    pub fn sanitized(self) -> Self {
        Self {
            stagger_damage_fraction: finite_or(self.stagger_damage_fraction, 0.20).clamp(0.0, 1.0),
            stagger_impulse_threshold: finite_or(self.stagger_impulse_threshold, 4.0)
                .clamp(0.0, 100_000.0),
            flinch_duration_seconds: finite_or(self.flinch_duration_seconds, 0.16).clamp(0.0, 10.0),
            stagger_duration_seconds: finite_or(self.stagger_duration_seconds, 0.42)
                .clamp(0.0, 10.0),
            injured_health_fraction: finite_or(self.injured_health_fraction, 0.30).clamp(0.0, 1.0),
        }
    }
}

impl Default for CharacterDamageResponseTuning {
    fn default() -> Self {
        Self {
            stagger_damage_fraction: 0.20,
            stagger_impulse_threshold: 4.0,
            flinch_duration_seconds: 0.16,
            stagger_duration_seconds: 0.42,
            injured_health_fraction: 0.30,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterHitReactionState {
    pub kind: CharacterHitReactionKind,
    pub remaining_seconds: f32,
    pub sequence: u64,
    pub source: u64,
    pub hit_zone: Option<String>,
    pub point: Vec3,
    pub impulse: Vec3,
    pub applied_damage: f32,
    pub health_fraction: f32,
    pub revision: u64,
}

impl Default for CharacterHitReactionState {
    fn default() -> Self {
        Self {
            kind: CharacterHitReactionKind::None,
            remaining_seconds: 0.0,
            sequence: 0,
            source: 0,
            hit_zone: None,
            point: Vec3::ZERO,
            impulse: Vec3::ZERO,
            applied_damage: 0.0,
            health_fraction: 1.0,
            revision: 0,
        }
    }
}

impl CharacterHitReactionState {
    #[inline]
    pub fn active(&self) -> bool {
        self.kind != CharacterHitReactionKind::None && self.remaining_seconds > 1.0e-6
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInjuryState {
    pub injured: bool,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterDeathPresentation {
    Animation,
    Ragdoll,
    #[default]
    AnimationThenRagdoll,
}

impl CharacterDeathPresentation {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Ragdoll => "ragdoll",
            Self::AnimationThenRagdoll => "animation_then_ragdoll",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterDeathPolicy {
    pub drop_active_weapon: bool,
    pub presentation: CharacterDeathPresentation,
}

impl Default for CharacterDeathPolicy {
    fn default() -> Self {
        Self {
            drop_active_weapon: false,
            presentation: CharacterDeathPresentation::AnimationThenRagdoll,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterDeathPhase {
    #[default]
    TransitionRequested,
    Corpse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDeathTransitionState {
    pub phase: CharacterDeathPhase,
    pub sequence: u64,
    pub source: u64,
    pub hit_zone: Option<String>,
    pub point: Vec3,
    pub impulse: Vec3,
    pub dropped_weapon_entity: Option<u64>,
    pub presentation: CharacterDeathPresentation,
    pub revision: u64,
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
    pub reaction: CharacterHitReactionKind,
    pub injured: bool,
    pub lethal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingPhysicsImpulse {
    pub sequence: u64,
    pub impulse: Vec3,
    pub point: Vec3,
}
