#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_ui_api::{UiEventDispatchFrame, UiStatePatch};

use super::execution::GameplayWorld;
use crate::provider_contract::{
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
                crate::ui_gateway::publish_state_patch(
                    &patch.patch,
                    patch.source_gateway,
                    patch.contract,
                );
            }
            for surface in output.surface_visibility {
                crate::ui_gateway::set_surface_visible(&surface.surface_id, surface.visible);
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
