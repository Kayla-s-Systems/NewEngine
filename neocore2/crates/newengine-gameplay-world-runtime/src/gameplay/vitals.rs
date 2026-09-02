use newengine_ecs::World;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(maximum: f32) -> Self {
        let maximum = finite_non_negative(maximum, 0.0);
        Self {
            current: maximum,
            maximum,
        }
    }

    #[inline]
    pub fn normalized(self) -> f32 {
        if self.maximum <= 1.0e-6 {
            0.0
        } else {
            (self.current / self.maximum).clamp(0.0, 1.0)
        }
    }

    pub fn apply_damage(&mut self, amount: f32) -> f32 {
        let amount = finite_non_negative(amount, 0.0);
        let maximum = finite_non_negative(self.maximum, 0.0);
        let before = finite_non_negative(self.current, 0.0).min(maximum);
        self.maximum = maximum;
        self.current = (before - amount).clamp(0.0, maximum);
        before - self.current
    }

    pub fn heal(&mut self, amount: f32) -> f32 {
        let amount = finite_non_negative(amount, 0.0);
        let maximum = finite_non_negative(self.maximum, 0.0);
        let before = finite_non_negative(self.current, 0.0).min(maximum);
        self.maximum = maximum;
        self.current = (before + amount).clamp(0.0, maximum);
        self.current - before
    }

    #[inline]
    pub fn alive(self) -> bool {
        self.current > 0.0
    }

    #[inline]
    pub fn depleted(self) -> bool {
        !self.alive()
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterLifeState {
    #[default]
    Alive,
    Dead,
}

impl CharacterLifeState {
    #[inline]
    pub const fn alive(self) -> bool {
        matches!(self, Self::Alive)
    }
}

/// Generic character-level control gate shared by local input, remote controllers and future AI.
/// Death disables this state; controller implementations must honor it before authoring movement
/// or combat intent. This avoids a player-only death contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterControlState {
    pub enabled: bool,
}

impl CharacterControlState {
    #[inline]
    pub const fn enabled() -> Self {
        Self { enabled: true }
    }

    #[inline]
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }
}

impl Default for CharacterControlState {
    fn default() -> Self {
        Self::enabled()
    }
}

/// Reconciles the explicit life-state component after an authoritative health mutation.
/// Ordinary healing never resurrects a dead character; a future revive mechanic must own that
/// transition explicitly. Returns `true` only for the first Alive -> Dead transition.
pub fn reconcile_character_life_state(world: &mut World, entity: newengine_ecs::EntityId) -> bool {
    let depleted = world
        .get::<Health>(entity)
        .copied()
        .is_some_and(Health::depleted);
    if !depleted {
        if world.get::<CharacterLifeState>(entity).is_none() {
            let _ = world.insert(entity, CharacterLifeState::Alive);
        }
        return false;
    }

    match world.get::<CharacterLifeState>(entity).copied() {
        Some(CharacterLifeState::Dead) => false,
        _ => {
            let _ = world.insert(entity, CharacterLifeState::Dead);
            true
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaminaTuning {
    /// Continuous stamina cost while the character is actually sprinting.
    pub sprint_drain_per_second: f32,
    /// Recovery rate after the post-exertion delay expires.
    pub regen_per_second: f32,
    /// Recovery is suppressed for this long after each stamina spend.
    pub regen_delay_seconds: f32,
    /// Exhaustion hysteresis: sprint remains locked until this fraction is restored.
    pub exhausted_resume_fraction: f32,
}

impl StaminaTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            sprint_drain_per_second: finite_non_negative(self.sprint_drain_per_second, 0.0)
                .clamp(0.0, 10_000.0),
            regen_per_second: finite_non_negative(self.regen_per_second, 0.0).clamp(0.0, 10_000.0),
            regen_delay_seconds: finite_non_negative(self.regen_delay_seconds, 0.0)
                .clamp(0.0, 60.0),
            exhausted_resume_fraction: finite_or(self.exhausted_resume_fraction, 0.20)
                .clamp(0.0, 1.0),
        }
    }
}

impl Default for StaminaTuning {
    fn default() -> Self {
        Self {
            sprint_drain_per_second: 16.0,
            regen_per_second: 22.0,
            regen_delay_seconds: 0.85,
            exhausted_resume_fraction: 0.20,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamina {
    pub current: f32,
    pub maximum: f32,
    /// Remaining delay before passive regeneration may begin.
    pub regen_delay_remaining: f32,
    /// Exhaustion is latched at zero and released only after the authored resume fraction.
    pub exhausted: bool,
}

impl Stamina {
    pub fn new(maximum: f32) -> Self {
        let maximum = finite_non_negative(maximum, 0.0);
        Self {
            current: maximum,
            maximum,
            regen_delay_remaining: 0.0,
            exhausted: maximum <= 1.0e-6,
        }
    }

    #[inline]
    pub fn normalized(self) -> f32 {
        if self.maximum <= 1.0e-6 {
            0.0
        } else {
            (self.current / self.maximum).clamp(0.0, 1.0)
        }
    }

    #[inline]
    pub fn can_sprint(self) -> bool {
        !self.exhausted && self.current > 1.0e-6 && self.maximum > 1.0e-6
    }

    pub fn spend(&mut self, amount: f32, tuning: StaminaTuning) -> f32 {
        let tuning = tuning.sanitized();
        let amount = finite_non_negative(amount, 0.0);
        let maximum = finite_non_negative(self.maximum, 0.0);
        let before = finite_non_negative(self.current, 0.0).min(maximum);
        self.maximum = maximum;
        self.current = (before - amount).clamp(0.0, maximum);
        if amount > 0.0 {
            self.regen_delay_remaining = tuning.regen_delay_seconds;
        }
        if self.current <= 1.0e-6 {
            self.current = 0.0;
            self.exhausted = true;
        }
        before - self.current
    }

    pub fn restore(&mut self, amount: f32, tuning: StaminaTuning) -> f32 {
        let tuning = tuning.sanitized();
        let amount = finite_non_negative(amount, 0.0);
        let maximum = finite_non_negative(self.maximum, 0.0);
        let before = finite_non_negative(self.current, 0.0).min(maximum);
        self.maximum = maximum;
        self.current = (before + amount).clamp(0.0, maximum);
        if self.exhausted
            && (self.maximum <= 1.0e-6
                || self.normalized() + 1.0e-6 >= tuning.exhausted_resume_fraction)
        {
            self.exhausted = self.maximum <= 1.0e-6;
        }
        self.current - before
    }

    pub fn step_recovery(&mut self, dt: f32, tuning: StaminaTuning) -> f32 {
        let tuning = tuning.sanitized();
        let dt = finite_non_negative(dt, 0.0);
        if dt <= 0.0 {
            return 0.0;
        }
        if self.regen_delay_remaining > 0.0 {
            self.regen_delay_remaining = (self.regen_delay_remaining - dt).max(0.0);
            return 0.0;
        }
        self.restore(tuning.regen_per_second * dt, tuning)
    }
}

impl Default for Stamina {
    fn default() -> Self {
        Self::new(100.0)
    }
}

/// Semantic exertion authored by a controller. The stamina system consumes this state at the
/// fixed-step boundary, so local input, remote input and future AI controllers share one contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterExertionState {
    pub sprinting: bool,
}

pub fn update_character_vitals(world: &mut World, dt: f32) {
    let dt = finite_non_negative(dt, 0.0);
    if dt <= 0.0 {
        return;
    }

    let entities = world
        .query2_ids::<Stamina, CharacterExertionState>()
        .collect::<Vec<_>>();
    for entity in entities {
        if world
            .get::<CharacterLifeState>(entity)
            .is_some_and(|state| !state.alive())
        {
            if let Some(exertion) = world.get_mut::<CharacterExertionState>(entity) {
                exertion.sprinting = false;
            }
            continue;
        }
        let exertion = world
            .get::<CharacterExertionState>(entity)
            .copied()
            .unwrap_or_default();
        let tuning = world
            .get::<StaminaTuning>(entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let (was_exhausted, is_exhausted, current, maximum) = {
            let Some(stamina) = world.get_mut::<Stamina>(entity) else {
                continue;
            };
            let was_exhausted = stamina.exhausted;
            if exertion.sprinting && stamina.can_sprint() {
                stamina.spend(tuning.sprint_drain_per_second * dt, tuning);
            } else {
                stamina.step_recovery(dt, tuning);
            }
            (
                was_exhausted,
                stamina.exhausted,
                stamina.current,
                stamina.maximum,
            )
        };
        if !was_exhausted && is_exhausted {
            let _ = super::emit_gameplay_event(
                world,
                super::GAMEPLAY_EVENT_CHARACTER_STAMINA_EXHAUSTED,
                Some(entity),
                serde_json::json!({
                    "stamina_current": current,
                    "stamina_maximum": maximum,
                    "stamina_normalized": 0.0,
                }),
            );
        } else if was_exhausted && !is_exhausted {
            let normalized = if maximum <= 1.0e-6 {
                0.0
            } else {
                (current / maximum).clamp(0.0, 1.0)
            };
            let _ = super::emit_gameplay_event(
                world,
                super::GAMEPLAY_EVENT_CHARACTER_STAMINA_RECOVERED,
                Some(entity),
                serde_json::json!({
                    "stamina_current": current,
                    "stamina_maximum": maximum,
                    "stamina_normalized": normalized,
                }),
            );
        }
    }
}

#[inline]
fn finite_non_negative(value: f32, fallback: f32) -> f32 {
    finite_or(value, fallback).max(0.0)
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_clamps_damage_and_healing() {
        let mut health = Health::new(100.0);
        assert_eq!(health.apply_damage(35.0), 35.0);
        assert_eq!(health.current, 65.0);
        assert_eq!(health.heal(20.0), 20.0);
        assert_eq!(health.current, 85.0);
        assert_eq!(health.heal(1000.0), 15.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.apply_damage(1000.0), 100.0);
        assert!(health.depleted());
    }

    #[test]
    fn stamina_exhaustion_uses_resume_hysteresis() {
        let tuning = StaminaTuning {
            sprint_drain_per_second: 20.0,
            regen_per_second: 20.0,
            regen_delay_seconds: 0.5,
            exhausted_resume_fraction: 0.25,
        };
        let mut stamina = Stamina::new(100.0);
        stamina.spend(100.0, tuning);
        assert!(stamina.exhausted);
        assert!(!stamina.can_sprint());
        assert_eq!(stamina.step_recovery(0.25, tuning), 0.0);
        assert_eq!(stamina.step_recovery(0.25, tuning), 0.0);
        stamina.step_recovery(1.0, tuning);
        assert_eq!(stamina.current, 20.0);
        assert!(stamina.exhausted);
        stamina.step_recovery(0.25, tuning);
        assert_eq!(stamina.current, 25.0);
        assert!(!stamina.exhausted);
        assert!(stamina.can_sprint());
    }

    #[test]
    fn stamina_exhaustion_and_recovery_publish_edge_events_once() {
        let mut world = World::new();
        let entity = world.spawn();
        let tuning = StaminaTuning {
            sprint_drain_per_second: 100.0,
            regen_per_second: 25.0,
            regen_delay_seconds: 0.0,
            exhausted_resume_fraction: 0.25,
        };
        let _ = world.insert(entity, Stamina::new(100.0));
        let _ = world.insert(entity, tuning);
        let _ = world.insert(entity, CharacterLifeState::Alive);
        let _ = world.insert(entity, CharacterExertionState { sprinting: true });

        update_character_vitals(&mut world, 1.0);
        let events = crate::gameplay::drain_gameplay_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].id,
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_STAMINA_EXHAUSTED
        );

        world
            .get_mut::<CharacterExertionState>(entity)
            .unwrap()
            .sprinting = false;
        update_character_vitals(&mut world, 1.0);
        let events = crate::gameplay::drain_gameplay_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].id,
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_STAMINA_RECOVERED
        );

        update_character_vitals(&mut world, 1.0);
        assert!(crate::gameplay::drain_gameplay_events(&mut world).is_empty());
    }

    #[test]
    fn fixed_step_vitals_drain_only_while_exerting() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(entity, Stamina::new(100.0));
        let _ = world.insert(
            entity,
            StaminaTuning {
                sprint_drain_per_second: 10.0,
                regen_per_second: 20.0,
                regen_delay_seconds: 0.5,
                exhausted_resume_fraction: 0.2,
            },
        );
        let _ = world.insert(entity, CharacterExertionState { sprinting: true });

        update_character_vitals(&mut world, 1.0);
        assert_eq!(world.get::<Stamina>(entity).unwrap().current, 90.0);

        world
            .get_mut::<CharacterExertionState>(entity)
            .unwrap()
            .sprinting = false;
        update_character_vitals(&mut world, 0.5);
        assert_eq!(world.get::<Stamina>(entity).unwrap().current, 90.0);
        update_character_vitals(&mut world, 0.5);
        assert_eq!(world.get::<Stamina>(entity).unwrap().current, 100.0);
    }
}
