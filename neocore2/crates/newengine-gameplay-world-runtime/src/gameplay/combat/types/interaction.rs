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
    MeleeAttacked,
    Empty,
    ReloadStarted,
    ReloadMagazineDetached,
    ReloadAmmoCommitted,
    ReloadMagazineInserted,
    ReloadChambered,
    ReloadCompleted,
    Hit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponEvent {
    pub kind: WeaponEventKind,
    pub shooter: EntityId,
    /// Concrete inventory weapon instance that authored this event.
    pub weapon_instance_id: ItemInstanceId,
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
