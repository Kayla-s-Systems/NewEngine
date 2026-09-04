/// Controller-neutral weapon actuation request consumed by product combat runtimes.
///
/// This component carries only actions. It never names a target, damage amount, projectile hit,
/// or health mutation, so player input and AI can converge on the same weapon state machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatActuationState {
    pub aim: bool,
    pub trigger_pressed: bool,
    pub trigger_held: bool,
    pub reload_pressed: bool,
    pub source_frame: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitscanWeaponTuning {
    pub magazine_capacity: u32,
    pub reserve_capacity: u32,
    pub fire_interval: f32,
    pub reload_duration: f32,
    pub damage: f32,
    pub range: f32,
    pub hip_spread_radians: f32,
    pub aim_spread_radians: f32,
    pub movement_spread_multiplier: f32,
    pub crouch_spread_multiplier: f32,
    pub recoil_accuracy_per_shot_radians: f32,
    pub recoil_accuracy_max_radians: f32,
    pub accuracy_recovery_hz: f32,
    pub accuracy_recovery_delay_seconds: f32,
    pub recoil_pitch_radians: f32,
    pub recoil_pitch_random_radians: f32,
    pub recoil_yaw_radians: f32,
    pub recoil_yaw_bias_radians: f32,
    pub ads_recoil_multiplier: f32,
    pub recoil_recovery_hz: f32,
    /// Initial recoil-tracker velocity relative to `kick * recovery_hz`. Kept separate from the
    /// angle impulse so authored weapons can control post-shot follow-through independently.
    pub recoil_pitch_tracker_speed_scale: f32,
    pub recoil_yaw_tracker_speed_scale: f32,
    pub muzzle_forward_offset: f32,
    /// NorthStar-style instant projectile may continue as one shallow-angle ricochet trace.
    pub ricochet_enabled: bool,
    pub ricochet_max_bounces: u8,
    /// Maximum absolute `dot(-shot_dir, hit_normal)` that is considered a grazing impact.
    pub ricochet_grazing_dot: f32,
    /// Damage/remaining-range energy retained by each bounce.
    pub ricochet_energy_retention: f32,
}

impl Default for HitscanWeaponTuning {
    fn default() -> Self {
        Self {
            magazine_capacity: 30,
            reserve_capacity: 90,
            fire_interval: 0.1,
            reload_duration: 1.8,
            damage: 25.0,
            range: 120.0,
            hip_spread_radians: 1.5_f32.to_radians(),
            aim_spread_radians: 0.25_f32.to_radians(),
            movement_spread_multiplier: 1.65,
            crouch_spread_multiplier: 0.82,
            recoil_accuracy_per_shot_radians: 0.28_f32.to_radians(),
            recoil_accuracy_max_radians: 3.5_f32.to_radians(),
            accuracy_recovery_hz: 5.5,
            accuracy_recovery_delay_seconds: 0.10,
            recoil_pitch_radians: 0.8_f32.to_radians(),
            recoil_pitch_random_radians: 0.15_f32.to_radians(),
            recoil_yaw_radians: 0.35_f32.to_radians(),
            recoil_yaw_bias_radians: 0.0,
            ads_recoil_multiplier: 0.78,
            recoil_recovery_hz: 7.5,
            recoil_pitch_tracker_speed_scale: 1.4,
            recoil_yaw_tracker_speed_scale: 1.15,
            muzzle_forward_offset: 0.52,
            ricochet_enabled: true,
            ricochet_max_bounces: 1,
            ricochet_grazing_dot: 0.38,
            ricochet_energy_retention: 0.38,
        }
    }
}

impl HitscanWeaponTuning {
    /// Shared finite center-screen convergence distance for ADS presentation and ballistics.
    /// Keeping this policy on the weapon tuning prevents character animation and combat from
    /// silently choosing different toe-in distances for the same firearm.
    #[inline]
    pub fn ads_center_screen_convergence_m(self) -> f32 {
        self.sanitized().range.clamp(12.0, 80.0)
    }

    pub fn sanitized(self) -> Self {
        Self {
            magazine_capacity: self.magazine_capacity.clamp(1, 10_000),
            reserve_capacity: self.reserve_capacity.min(1_000_000),
            fire_interval: self.fire_interval.clamp(0.01, 60.0),
            reload_duration: self.reload_duration.clamp(0.0, 120.0),
            damage: self.damage.clamp(0.0, 1_000_000.0),
            // NorthStar's bounded mesh-ray path asserts on rays longer than 1000 m. Keep the same
            // upper contract for instant firearm projectiles.
            range: self.range.clamp(0.1, 1_000.0),
            hip_spread_radians: self
                .hip_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            aim_spread_radians: self
                .aim_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            movement_spread_multiplier: self.movement_spread_multiplier.clamp(1.0, 8.0),
            crouch_spread_multiplier: self.crouch_spread_multiplier.clamp(0.1, 2.0),
            recoil_accuracy_per_shot_radians: self
                .recoil_accuracy_per_shot_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_4),
            recoil_accuracy_max_radians: self
                .recoil_accuracy_max_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            accuracy_recovery_hz: self.accuracy_recovery_hz.clamp(0.05, 120.0),
            accuracy_recovery_delay_seconds: self.accuracy_recovery_delay_seconds.clamp(0.0, 5.0),
            recoil_pitch_radians: self.recoil_pitch_radians.clamp(0.0, 1.0),
            recoil_pitch_random_radians: self.recoil_pitch_random_radians.clamp(0.0, 1.0),
            recoil_yaw_radians: self.recoil_yaw_radians.clamp(0.0, 1.0),
            recoil_yaw_bias_radians: self.recoil_yaw_bias_radians.clamp(-1.0, 1.0),
            ads_recoil_multiplier: self.ads_recoil_multiplier.clamp(0.0, 4.0),
            recoil_recovery_hz: self.recoil_recovery_hz.clamp(0.05, 120.0),
            recoil_pitch_tracker_speed_scale: self.recoil_pitch_tracker_speed_scale.clamp(0.0, 4.0),
            recoil_yaw_tracker_speed_scale: self.recoil_yaw_tracker_speed_scale.clamp(0.0, 4.0),
            muzzle_forward_offset: self.muzzle_forward_offset.clamp(0.0, 10.0),
            ricochet_enabled: self.ricochet_enabled,
            ricochet_max_bounces: self.ricochet_max_bounces.min(4),
            ricochet_grazing_dot: self.ricochet_grazing_dot.clamp(0.0, 0.95),
            ricochet_energy_retention: self.ricochet_energy_retention.clamp(0.0, 0.95),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFireControllerState {
    pub weapon_instance_id: ItemInstanceId,
    pub trigger_was_held: bool,
    pub activation_seconds: f32,
    pub pattern_cooldown_seconds: f32,
    pub burst_shots_remaining: u8,
    pub bursts_remaining: u8,
}

impl WeaponFireControllerState {
    #[inline]
    pub fn new(weapon_instance_id: ItemInstanceId) -> Self {
        Self {
            weapon_instance_id,
            trigger_was_held: false,
            activation_seconds: 0.0,
            pattern_cooldown_seconds: 0.0,
            burst_shots_remaining: 0,
            bursts_remaining: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponAccuracyState {
    pub weapon_instance_id: ItemInstanceId,
    pub bloom_radians: f32,
    pub recovery_velocity: f32,
    pub shot_count: u32,
    pub time_since_shot: f32,
}

impl WeaponAccuracyState {
    #[inline]
    pub fn new(weapon_instance_id: ItemInstanceId) -> Self {
        Self {
            weapon_instance_id,
            bloom_radians: 0.0,
            recovery_velocity: 0.0,
            shot_count: 0,
            time_since_shot: f32::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponAccuracyModifiers {
    pub component_multiplier: f32,
    pub character_multiplier: f32,
}

impl Default for WeaponAccuracyModifiers {
    fn default() -> Self {
        Self {
            component_multiplier: 1.0,
            character_multiplier: 1.0,
        }
    }
}

impl WeaponAccuracyModifiers {
    #[inline]
    pub fn combined(self) -> f32 {
        let component = if self.component_multiplier.is_finite() {
            self.component_multiplier.clamp(0.1, 8.0)
        } else {
            1.0
        };
        let character = if self.character_multiplier.is_finite() {
            self.character_multiplier.clamp(0.1, 8.0)
        } else {
            1.0
        };
        component * character
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponActionKind {
    #[default]
    Ready,
    Firing,
    Reloading,
    Cycling,
    Melee,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponReloadPhase {
    #[default]
    None,
    Started,
    MagazineDetached,
    AmmoCommitted,
    MagazineInserted,
    Chambered,
    Complete,
}

pub const WEAPON_RELOAD_MARKER_MAGAZINE_DETACHED: &str = "weapon.mag.detach";
pub const WEAPON_RELOAD_MARKER_AMMO_COMMITTED: &str = "weapon.ammo.commit";
pub const WEAPON_RELOAD_MARKER_MAGAZINE_INSERTED: &str = "weapon.mag.insert";
pub const WEAPON_RELOAD_MARKER_CHAMBERED: &str = "weapon.chamber";
pub const WEAPON_RELOAD_MARKER_COMPLETE: &str = "weapon.reload.complete";
pub const WEAPON_RELOAD_ANIMATION_REQUIRED_MARKER_MASK: u8 = 0b1_1111;

impl WeaponReloadPhase {
    #[inline]
    pub const fn marker_bit(self) -> u8 {
        match self {
            Self::MagazineDetached => 1 << 0,
            Self::AmmoCommitted => 1 << 1,
            Self::MagazineInserted => 1 << 2,
            Self::Chambered => 1 << 3,
            Self::Complete => 1 << 4,
            Self::None | Self::Started => 0,
        }
    }

    #[inline]
    pub const fn animation_marker_tag(self) -> Option<&'static str> {
        match self {
            Self::MagazineDetached => Some(WEAPON_RELOAD_MARKER_MAGAZINE_DETACHED),
            Self::AmmoCommitted => Some(WEAPON_RELOAD_MARKER_AMMO_COMMITTED),
            Self::MagazineInserted => Some(WEAPON_RELOAD_MARKER_MAGAZINE_INSERTED),
            Self::Chambered => Some(WEAPON_RELOAD_MARKER_CHAMBERED),
            Self::Complete => Some(WEAPON_RELOAD_MARKER_COMPLETE),
            Self::None | Self::Started => None,
        }
    }

    pub fn from_animation_marker_tag(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            WEAPON_RELOAD_MARKER_MAGAZINE_DETACHED => Some(Self::MagazineDetached),
            WEAPON_RELOAD_MARKER_AMMO_COMMITTED => Some(Self::AmmoCommitted),
            WEAPON_RELOAD_MARKER_MAGAZINE_INSERTED => Some(Self::MagazineInserted),
            WEAPON_RELOAD_MARKER_CHAMBERED => Some(Self::Chambered),
            WEAPON_RELOAD_MARKER_COMPLETE => Some(Self::Complete),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponActionTimingSource {
    #[default]
    TimelineFallback,
    AnimationMarkers,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponReloadAnimationAuthority {
    pub weapon_instance_id: ItemInstanceId,
    pub clip_duration_seconds: f32,
    pub marker_mask: u8,
    pub required_marker_mask: u8,
}

impl WeaponReloadAnimationAuthority {
    #[inline]
    pub fn is_complete(self) -> bool {
        self.clip_duration_seconds.is_finite()
            && self.clip_duration_seconds > 1.0e-4
            && self.required_marker_mask != 0
            && self.marker_mask & self.required_marker_mask == self.required_marker_mask
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponReloadAnimationMarker {
    pub weapon_instance_id: ItemInstanceId,
    pub phase: WeaponReloadPhase,
    pub clip_time_seconds: f32,
    pub playback_time_seconds: f32,
    pub loop_index: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponReloadAnimationMarkerInbox {
    pub markers: Vec<WeaponReloadAnimationMarker>,
}

impl WeaponReloadAnimationMarkerInbox {
    pub const MAX_RETAINED_MARKERS: usize = 64;

    pub fn push(&mut self, marker: WeaponReloadAnimationMarker) {
        if self.markers.len() >= Self::MAX_RETAINED_MARKERS {
            let overflow = self.markers.len() + 1 - Self::MAX_RETAINED_MARKERS;
            self.markers.drain(0..overflow);
        }
        self.markers.push(marker);
    }

    pub fn drain_for_instance(
        &mut self,
        weapon_instance_id: ItemInstanceId,
    ) -> Vec<WeaponReloadAnimationMarker> {
        let mut matched = Vec::new();
        self.markers.retain(|marker| {
            if marker.weapon_instance_id == weapon_instance_id {
                matched.push(*marker);
                false
            } else {
                true
            }
        });
        matched
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponActionRuntime {
    pub weapon_instance_id: ItemInstanceId,
    pub action: WeaponActionKind,
    pub reload_phase: WeaponReloadPhase,
    pub timing_source: WeaponActionTimingSource,
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub phase_mask: u8,
}

impl WeaponActionRuntime {
    #[inline]
    pub fn ready(weapon_instance_id: ItemInstanceId) -> Self {
        Self {
            weapon_instance_id,
            action: WeaponActionKind::Ready,
            reload_phase: WeaponReloadPhase::None,
            timing_source: WeaponActionTimingSource::TimelineFallback,
            elapsed_seconds: 0.0,
            duration_seconds: 0.0,
            phase_mask: 0,
        }
    }

    #[inline]
    pub fn begin_reload(
        weapon_instance_id: ItemInstanceId,
        duration_seconds: f32,
        timing_source: WeaponActionTimingSource,
    ) -> Self {
        Self {
            weapon_instance_id,
            action: WeaponActionKind::Reloading,
            reload_phase: WeaponReloadPhase::Started,
            timing_source,
            elapsed_seconds: 0.0,
            duration_seconds: duration_seconds.max(0.0),
            phase_mask: 0,
        }
    }

    #[inline]
    pub fn progress(self) -> f32 {
        if self.duration_seconds <= 1.0e-6 {
            1.0
        } else {
            (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerWeaponState {
    pub ammo_in_magazine: u32,
    pub reserve_ammo: u32,
    pub cooldown_remaining: f32,
    pub reload_remaining: f32,
    pub shot_sequence: u64,
    pub aiming: bool,
    pub empty_latched: bool,
}

impl PlayerWeaponState {
    pub fn loaded(tuning: HitscanWeaponTuning) -> Self {
        let tuning = tuning.sanitized();
        Self {
            ammo_in_magazine: tuning.magazine_capacity,
            reserve_ammo: tuning.reserve_capacity,
            cooldown_remaining: 0.0,
            reload_remaining: 0.0,
            shot_sequence: 0,
            aiming: false,
            empty_latched: false,
        }
    }

    pub const fn melee() -> Self {
        Self {
            ammo_in_magazine: 0,
            reserve_ammo: 0,
            cooldown_remaining: 0.0,
            reload_remaining: 0.0,
            shot_sequence: 0,
            aiming: false,
            empty_latched: false,
        }
    }
}

impl Default for PlayerWeaponState {
    fn default() -> Self {
        Self::loaded(HitscanWeaponTuning::default())
    }
}

/// Latest fixed-step weapon/body obstruction probe. The equipped-weapon presentation consumes
/// this as a physical constraint: the firing hand remains the primary joint, while the barrel is
/// raised/retracted before it can cross solid world geometry. Ballistics also use `safe_muzzle_position`
/// so a muzzle that was visually beyond a wall can never spawn a ray on the far side of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponObstructionState {
    pub blocked: bool,
    /// Normalized penetration/clearance pressure in `[0, 1]`.
    pub alpha: f32,
    pub hit_position: Vec3,
    pub hit_normal: Vec3,
    pub safe_muzzle_position: Vec3,
    pub fixed_tick: u64,
}

impl WeaponObstructionState {
    #[inline]
    pub fn clear(muzzle_position: Vec3, fixed_tick: u64) -> Self {
        Self {
            blocked: false,
            alpha: 0.0,
            hit_position: muzzle_position,
            hit_normal: Vec3::ZERO,
            safe_muzzle_position: muzzle_position,
            fixed_tick,
        }
    }
}

impl Default for WeaponObstructionState {
    fn default() -> Self {
        Self::clear(Vec3::ZERO, 0)
    }
}
