use newengine_ecs::EntityId;
use newengine_math::Vec3;

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
    pub recoil_pitch_radians: f32,
    pub recoil_yaw_radians: f32,
    pub muzzle_forward_offset: f32,
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
            recoil_pitch_radians: 0.8_f32.to_radians(),
            recoil_yaw_radians: 0.35_f32.to_radians(),
            muzzle_forward_offset: 0.52,
        }
    }
}

impl HitscanWeaponTuning {
    pub fn sanitized(self) -> Self {
        Self {
            magazine_capacity: self.magazine_capacity.clamp(1, 10_000),
            reserve_capacity: self.reserve_capacity.min(1_000_000),
            fire_interval: self.fire_interval.clamp(0.01, 60.0),
            reload_duration: self.reload_duration.clamp(0.0, 120.0),
            damage: self.damage.clamp(0.0, 1_000_000.0),
            range: self.range.clamp(0.1, 100_000.0),
            hip_spread_radians: self
                .hip_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            aim_spread_radians: self
                .aim_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            recoil_pitch_radians: self.recoil_pitch_radians.clamp(0.0, 1.0),
            recoil_yaw_radians: self.recoil_yaw_radians.clamp(0.0, 1.0),
            muzzle_forward_offset: self.muzzle_forward_offset.clamp(0.0, 10.0),
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
}

impl Default for PlayerWeaponState {
    fn default() -> Self {
        Self::loaded(HitscanWeaponTuning::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(maximum: f32) -> Self {
        let maximum = maximum.max(0.0);
        Self {
            current: maximum,
            maximum,
        }
    }

    pub fn apply_damage(&mut self, amount: f32) -> f32 {
        let amount = if amount.is_finite() {
            amount.max(0.0)
        } else {
            0.0
        };
        let before = self.current;
        self.current = (self.current - amount).clamp(0.0, self.maximum.max(0.0));
        before - self.current
    }

    #[inline]
    pub fn alive(self) -> bool {
        self.current > 0.0
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interactable {
    pub prompt: String,
    pub enabled: bool,
}

impl Interactable {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerInteractionTuning {
    pub range: f32,
    pub ray_origin_forward_offset: f32,
}

impl Default for PlayerInteractionTuning {
    fn default() -> Self {
        Self {
            range: 3.0,
            ray_origin_forward_offset: 0.52,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponEventKind {
    Fired,
    Empty,
    ReloadStarted,
    ReloadCompleted,
    Hit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponEvent {
    pub kind: WeaponEventKind,
    pub shooter: EntityId,
    pub target: Option<EntityId>,
    pub shot_sequence: u64,
    pub damage: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct WeaponEventBus {
    pub events: Vec<WeaponEvent>,
}

impl WeaponEventBus {
    pub fn emit(&mut self, event: WeaponEvent) {
        const CAPACITY: usize = 512;
        if self.events.len() >= CAPACITY {
            let overflow = self.events.len() + 1 - CAPACITY;
            self.events.drain(0..overflow);
        }
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<WeaponEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionEvent {
    pub player: EntityId,
    pub target: EntityId,
    pub prompt: String,
    pub fixed_tick: u64,
    pub point: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct InteractionEventBus {
    pub events: Vec<InteractionEvent>,
}

impl InteractionEventBus {
    pub fn emit(&mut self, event: InteractionEvent) {
        const CAPACITY: usize = 256;
        if self.events.len() >= CAPACITY {
            let overflow = self.events.len() + 1 - CAPACITY;
            self.events.drain(0..overflow);
        }
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<InteractionEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingHitscan {
    pub query_seq: u64,
    pub shot_sequence: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub damage: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingInteraction {
    pub query_seq: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
}
