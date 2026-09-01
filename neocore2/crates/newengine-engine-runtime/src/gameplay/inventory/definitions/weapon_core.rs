#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponFireMode {
    #[default]
    SemiAuto,
    Automatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FiringPatternKind {
    #[default]
    Semi,
    Automatic,
    Burst,
    Charge,
    SpinUp,
    Pump,
    BoltAction,
    Binary,
    ScriptedSequence,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiringPatternDefinition {
    pub kind: FiringPatternKind,
    pub bursts_min: u8,
    pub bursts_max: u8,
    pub shots_per_burst_min: u8,
    pub shots_per_burst_max: u8,
    pub time_between_shots: f32,
    pub time_between_bursts: f32,
    pub delay_before_firing: f32,
    pub burst_cooldown: f32,
}

impl Default for FiringPatternDefinition {
    fn default() -> Self {
        Self::from_fire_mode(WeaponFireMode::SemiAuto, 0.1)
    }
}

impl FiringPatternDefinition {
    pub fn from_fire_mode(mode: WeaponFireMode, fire_interval: f32) -> Self {
        Self {
            kind: match mode {
                WeaponFireMode::SemiAuto => FiringPatternKind::Semi,
                WeaponFireMode::Automatic => FiringPatternKind::Automatic,
            },
            bursts_min: 1,
            bursts_max: 1,
            shots_per_burst_min: 1,
            shots_per_burst_max: 1,
            time_between_shots: fire_interval,
            time_between_bursts: 0.0,
            delay_before_firing: 0.0,
            burst_cooldown: 0.0,
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.bursts_min = self.bursts_min.clamp(1, 32);
        self.bursts_max = self.bursts_max.clamp(self.bursts_min, 32);
        self.shots_per_burst_min = self.shots_per_burst_min.clamp(1, 64);
        self.shots_per_burst_max = self.shots_per_burst_max.clamp(self.shots_per_burst_min, 64);
        self.time_between_shots = finite_or(self.time_between_shots, 0.1).clamp(0.01, 60.0);
        self.time_between_bursts = finite_or(self.time_between_bursts, 0.0).clamp(0.0, 60.0);
        self.delay_before_firing = finite_or(self.delay_before_firing, 0.0).clamp(0.0, 60.0);
        self.burst_cooldown = finite_or(self.burst_cooldown, 0.0).clamp(0.0, 60.0);
        self
    }
}

/// Coarse weapon taxonomy. Ammo is deliberately not part of weapon identity: both Unarmed and
/// Melee are weapons, but neither requires ammunition or exposes firearm ADS/fire/reload actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponType {
    #[default]
    Unarmed,
    Melee,
    Firearm,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponCapabilities {
    pub melee: bool,
    pub fire: bool,
    pub aim: bool,
    pub reload: bool,
    pub uses_ammo: bool,
}

impl WeaponType {
    #[inline]
    pub const fn capabilities(self) -> WeaponCapabilities {
        match self {
            Self::Unarmed | Self::Melee => WeaponCapabilities {
                melee: true,
                fire: false,
                aim: false,
                reload: false,
                uses_ammo: false,
            },
            Self::Firearm => WeaponCapabilities {
                melee: false,
                fire: true,
                aim: true,
                reload: true,
                uses_ammo: true,
            },
        }
    }

    /// Default selection/order rank. Rank is classification priority only; it never changes damage.
    #[inline]
    pub const fn default_rank(self) -> u16 {
        match self {
            Self::Unarmed => 0,
            Self::Melee => 100,
            Self::Firearm => 200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeleeWeaponTuning {
    pub damage: f32,
    pub range: f32,
    pub attack_interval: f32,
}

impl MeleeWeaponTuning {
    pub fn sanitized(self) -> Self {
        Self {
            damage: sanitize_non_negative(self.damage).clamp(0.0, 10_000.0),
            range: sanitize_non_negative(self.range).clamp(0.1, 8.0),
            attack_interval: sanitize_non_negative(self.attack_interval).clamp(0.05, 10.0),
        }
    }
}

impl Default for MeleeWeaponTuning {
    fn default() -> Self {
        Self {
            damage: 18.0,
            range: 1.35,
            attack_interval: 0.45,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirearmWeaponDefinition {
    pub tuning: HitscanWeaponTuning,
    pub ammo_item: ItemId,
    /// Compatibility shorthand retained for old consumers; runtime firing uses `firing_pattern`.
    pub fire_mode: WeaponFireMode,
    pub firing_pattern: FiringPatternDefinition,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponItemDefinition {
    pub weapon_type: WeaponType,
    pub rank: u16,
    pub melee: Option<MeleeWeaponTuning>,
    pub firearm: Option<FirearmWeaponDefinition>,
}

impl WeaponItemDefinition {
    #[inline]
    pub fn unarmed(rank: u16, tuning: MeleeWeaponTuning) -> Self {
        Self {
            weapon_type: WeaponType::Unarmed,
            rank,
            melee: Some(tuning.sanitized()),
            firearm: None,
        }
    }

    #[inline]
    pub fn melee(rank: u16, tuning: MeleeWeaponTuning) -> Self {
        Self {
            weapon_type: WeaponType::Melee,
            rank,
            melee: Some(tuning.sanitized()),
            firearm: None,
        }
    }

    #[inline]
    pub fn firearm(
        rank: u16,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
    ) -> Self {
        Self {
            weapon_type: WeaponType::Firearm,
            rank,
            melee: None,
            firearm: Some(FirearmWeaponDefinition {
                tuning: tuning.sanitized(),
                ammo_item,
                fire_mode,
                firing_pattern: FiringPatternDefinition::from_fire_mode(
                    fire_mode,
                    tuning.fire_interval,
                ),
            }),
        }
    }

    #[inline]
    pub fn firearm_with_pattern(
        rank: u16,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
        firing_pattern: FiringPatternDefinition,
    ) -> Self {
        let mut weapon = Self::firearm(rank, tuning, ammo_item, fire_mode);
        if let Some(firearm) = weapon.firearm.as_mut() {
            firearm.firing_pattern = firing_pattern.sanitized();
        }
        weapon
    }

    #[inline]
    pub const fn capabilities(self) -> WeaponCapabilities {
        self.weapon_type.capabilities()
    }
}
