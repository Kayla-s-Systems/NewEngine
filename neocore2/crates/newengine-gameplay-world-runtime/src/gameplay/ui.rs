#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_ecs::EntityId;
use newengine_ui_api::{UiEventDispatchFrame, UiStatePatch};

use super::execution::GameplayWorld;
use newengine_runtime_provider_api::{
    validate_provider_contract, RuntimeProviderDescriptor, I_GAMEPLAY_UI_PROVIDER_V1,
    PROVIDER_CONTRACT_V1,
};

pub use newengine_input_capture_api::GameplayInputCapture;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameplayModalState {
    pub active: bool,
    pub capture: GameplayInputCapture,
    pub provider_count: u32,
    pub revision: u64,
}

impl GameplayModalState {
    #[inline]
    fn from_capture(capture: GameplayInputCapture, provider_count: usize, revision: u64) -> Self {
        Self {
            active: capture.requests_capture(),
            capture,
            provider_count: provider_count.min(u32::MAX as usize) as u32,
            revision,
        }
    }
}

#[derive(Debug)]
pub struct GameplayUiStatePatch {
    pub patch: UiStatePatch,
    pub source_gateway: &'static str,
    pub contract: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameplayUiSurfaceVisibility {
    pub surface_id: String,
    pub visible: bool,
}

#[derive(Debug, Default)]
pub struct GameplayUiFrameOutput {
    pub patches: Vec<GameplayUiStatePatch>,
    pub surface_visibility: Vec<GameplayUiSurfaceVisibility>,
}

impl GameplayUiFrameOutput {
    #[inline]
    pub fn with_patch(
        mut self,
        patch: UiStatePatch,
        source_gateway: &'static str,
        contract: &'static str,
    ) -> Self {
        self.patches.push(GameplayUiStatePatch {
            patch,
            source_gateway,
            contract,
        });
        self
    }

    #[inline]
    pub fn with_surface_visibility(mut self, surface_id: impl Into<String>, visible: bool) -> Self {
        self.surface_visibility.push(GameplayUiSurfaceVisibility {
            surface_id: surface_id.into(),
            visible,
        });
        self
    }
}

/// Profile-owned gameplay UI boundary.
///
/// The engine owns dispatch ordering, gateway publication and generic input-capture
/// semantics. Concrete HUD state, node/action ids and presentation data belong to
/// gameplay/profile crates.
pub trait GameplayUiProvider: Send + Sync {
    fn id(&self) -> &'static str;

    #[inline]
    fn descriptor(&self) -> RuntimeProviderDescriptor {
        RuntimeProviderDescriptor::gameplay_ui(self.id())
    }

    #[inline]
    fn dispatch_actions(&self, _world: &mut GameplayWorld, _frame: &UiEventDispatchFrame) -> bool {
        false
    }

    #[inline]
    fn publish_frame(
        &self,
        _world: &mut GameplayWorld,
        _frame_index: u64,
    ) -> GameplayUiFrameOutput {
        GameplayUiFrameOutput::default()
    }

    #[inline]
    fn input_capture(&self, _world: &GameplayWorld) -> GameplayInputCapture {
        GameplayInputCapture::none()
    }

    #[inline]
    fn reset_transient_state(&self, _world: &mut GameplayWorld) {}
}

#[derive(Default)]
pub struct GameplayUiProviderRegistry {
    providers: Vec<Arc<dyn GameplayUiProvider>>,
}

impl GameplayUiProviderRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(&mut self, provider: Arc<dyn GameplayUiProvider>) {
        let descriptor = provider.descriptor();
        if let Err(error) =
            validate_provider_contract(descriptor, I_GAMEPLAY_UI_PROVIDER_V1, PROVIDER_CONTRACT_V1)
        {
            newengine_ulog_api::ulog::warn!("gameplay UI provider rejected: {}", error);
            return;
        }
        let id = descriptor.id;
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id() == id)
        {
            *existing = provider;
            return;
        }
        self.providers.push(provider);
    }

    pub fn descriptors(&self) -> Vec<RuntimeProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }

    pub fn dispatch_actions(
        &self,
        world: &mut GameplayWorld,
        frame: &UiEventDispatchFrame,
    ) -> bool {
        let mut consumed = false;
        for provider in &self.providers {
            consumed |= provider.dispatch_actions(world, frame);
        }
        self.sync_modal_state(world);
        consumed
    }

    /// Publishes retained gameplay UI mutations and returns whether provider draw
    /// state changed. The caller uses this edge to invalidate the host-side game
    /// UI layer cache on the next platform frame.
    pub fn publish_frame(&self, world: &mut GameplayWorld, frame_index: u64) -> bool {
        let mut changed = false;
        for provider in &self.providers {
            let output = provider.publish_frame(world, frame_index);
            changed |= !output.patches.is_empty() || !output.surface_visibility.is_empty();
            for patch in output.patches {
                newengine_ui_client::publish_state_patch(
                    &patch.patch,
                    patch.source_gateway,
                    patch.contract,
                );
            }
            for surface in output.surface_visibility {
                newengine_ui_client::set_surface_visible(&surface.surface_id, surface.visible);
            }
        }
        self.sync_modal_state(world);
        changed
    }

    pub fn aggregate_input_capture(&self, world: &GameplayWorld) -> GameplayInputCapture {
        let mut capture = GameplayInputCapture::none();
        for provider in &self.providers {
            capture.merge(provider.input_capture(world));
        }
        capture
    }

    pub fn sync_modal_state(&self, world: &mut GameplayWorld) -> GameplayModalState {
        let capture = self.aggregate_input_capture(world);
        let provider_count = self
            .providers
            .iter()
            .filter(|provider| provider.input_capture(world).requests_capture())
            .count();
        let previous = world
            .resource::<GameplayModalState>()
            .copied()
            .unwrap_or_default();
        let changed = previous.capture != capture
            || previous.active != capture.requests_capture()
            || previous.provider_count != provider_count.min(u32::MAX as usize) as u32;
        let revision = if changed {
            previous.revision.wrapping_add(1)
        } else {
            previous.revision
        };
        let state = GameplayModalState::from_capture(capture, provider_count, revision);
        world.insert_resource(state);
        state
    }

    pub fn reset_transient_state(&self, world: &mut GameplayWorld) {
        for provider in &self.providers {
            provider.reset_transient_state(world);
        }
        self.sync_modal_state(world);
    }
}

#[inline]
pub fn gameplay_modal_state(world: &GameplayWorld) -> GameplayModalState {
    world
        .resource::<GameplayModalState>()
        .copied()
        .unwrap_or_default()
}

#[inline]
pub fn gameplay_input_capture(world: &GameplayWorld) -> GameplayInputCapture {
    gameplay_modal_state(world).capture
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterVitalsHudModel {
    pub entity: u64,
    pub alive: bool,
    pub control_enabled: bool,
    pub health_current: f32,
    pub health_maximum: f32,
    pub health_normalized: f32,
    pub stamina_available: bool,
    pub stamina_current: f32,
    pub stamina_maximum: f32,
    pub stamina_normalized: f32,
    pub stamina_exhausted: bool,
    pub injured: bool,
    pub hit_reaction: super::CharacterHitReactionKind,
    pub damage_flash: bool,
    pub death_phase: Option<super::CharacterDeathPhase>,
}

/// Read-only projection of authoritative character ECS state for HUD providers.
/// No UI surface owns or mutates health/stamina/life-state through this contract.
pub fn character_vitals_hud_model(
    world: &GameplayWorld,
    entity: EntityId,
) -> Option<CharacterVitalsHudModel> {
    let health = world.get::<super::Health>(entity).copied()?;
    let stamina = world.get::<super::Stamina>(entity).copied();
    let life_state = world
        .get::<super::CharacterLifeState>(entity)
        .copied()
        .unwrap_or_default();
    let control_enabled = world
        .get::<super::CharacterControlState>(entity)
        .map(|control| control.enabled)
        .unwrap_or(true);
    let injury = world
        .get::<super::CharacterInjuryState>(entity)
        .copied()
        .unwrap_or_default();
    let reaction = world.get::<super::CharacterHitReactionState>(entity);
    let death_phase = world
        .get::<super::CharacterDeathTransitionState>(entity)
        .map(|death| death.phase);

    Some(CharacterVitalsHudModel {
        entity: entity.stable_u64(),
        alive: life_state.alive(),
        control_enabled,
        health_current: health.current,
        health_maximum: health.maximum,
        health_normalized: health.normalized(),
        stamina_available: stamina.is_some(),
        stamina_current: stamina.map_or(0.0, |stamina| stamina.current),
        stamina_maximum: stamina.map_or(0.0, |stamina| stamina.maximum),
        stamina_normalized: stamina.map_or(0.0, |stamina| stamina.normalized()),
        stamina_exhausted: stamina.is_some_and(|stamina| stamina.exhausted),
        injured: injury.injured,
        hit_reaction: reaction
            .map(|reaction| reaction.kind)
            .unwrap_or(super::CharacterHitReactionKind::None),
        damage_flash: reaction.is_some_and(super::CharacterHitReactionState::active),
        death_phase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestUiProvider {
        id: &'static str,
        capture: GameplayInputCapture,
    }

    impl GameplayUiProvider for TestUiProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn input_capture(&self, _world: &GameplayWorld) -> GameplayInputCapture {
            self.capture
        }
    }

    #[test]
    fn character_vitals_hud_projection_is_read_only_and_reflects_authoritative_state() {
        let mut world = GameplayWorld::new();
        let entity = world.spawn();
        let mut stamina = super::super::Stamina::new(100.0);
        stamina.current = 25.0;
        stamina.exhausted = true;
        let _ = world.insert(
            entity,
            super::super::Health {
                current: 40.0,
                maximum: 100.0,
            },
        );
        let _ = world.insert(entity, stamina);
        let _ = world.insert(entity, super::super::CharacterLifeState::Alive);
        let _ = world.insert(entity, super::super::CharacterControlState::enabled());
        let _ = world.insert(
            entity,
            super::super::CharacterInjuryState {
                injured: true,
                revision: 2,
            },
        );
        let _ = world.insert(
            entity,
            super::super::CharacterHitReactionState {
                kind: super::super::CharacterHitReactionKind::Flinch,
                remaining_seconds: 0.1,
                sequence: 7,
                source: 8,
                hit_zone: Some("torso".to_owned()),
                point: newengine_math::Vec3::ZERO,
                impulse: newengine_math::Vec3::ZERO,
                applied_damage: 10.0,
                health_fraction: 0.4,
                revision: 3,
            },
        );

        let before_health = world.get::<super::super::Health>(entity).copied().unwrap();
        let model = character_vitals_hud_model(&world, entity).expect("HUD vitals model");
        assert_eq!(model.health_current, 40.0);
        assert_eq!(model.health_normalized, 0.4);
        assert_eq!(model.stamina_current, 25.0);
        assert_eq!(model.stamina_normalized, 0.25);
        assert!(model.stamina_exhausted);
        assert!(model.injured);
        assert!(model.damage_flash);
        assert_eq!(
            model.hit_reaction,
            super::super::CharacterHitReactionKind::Flinch
        );
        assert_eq!(
            world.get::<super::super::Health>(entity).copied(),
            Some(before_health)
        );
    }

    #[test]
    fn registry_merges_capture_across_gameplay_ui_providers() {
        let mut registry = GameplayUiProviderRegistry::new();
        registry.register_provider(Arc::new(TestUiProvider {
            id: "test.pointer",
            capture: GameplayInputCapture {
                pointer: true,
                release_cursor: true,
                ..GameplayInputCapture::none()
            },
        }));
        registry.register_provider(Arc::new(TestUiProvider {
            id: "test.movement",
            capture: GameplayInputCapture {
                block_player_movement: true,
                block_camera_navigation: true,
                ..GameplayInputCapture::none()
            },
        }));

        let mut world = GameplayWorld::new();
        let state = registry.sync_modal_state(&mut world);

        assert!(state.active);
        assert_eq!(state.provider_count, 2);
        assert!(state.capture.pointer);
        assert!(state.capture.release_cursor);
        assert!(state.capture.block_player_movement);
        assert!(state.capture.block_camera_navigation);
    }

    #[test]
    fn registering_same_ui_provider_id_replaces_policy_deterministically() {
        let mut registry = GameplayUiProviderRegistry::new();
        registry.register_provider(Arc::new(TestUiProvider {
            id: "test.modal",
            capture: GameplayInputCapture::modal(),
        }));
        registry.register_provider(Arc::new(TestUiProvider {
            id: "test.modal",
            capture: GameplayInputCapture::none(),
        }));

        let mut world = GameplayWorld::new();
        let state = registry.sync_modal_state(&mut world);

        assert!(!state.active);
        assert_eq!(state.provider_count, 0);
        assert_eq!(state.capture, GameplayInputCapture::none());
    }
}
