#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_ecs::World;
use newengine_sim::SimFrame;

use crate::provider_contract::{
    validate_provider_contract, RuntimeProviderDescriptor, I_GAMEPLAY_SYSTEM_PROVIDER_V1,
    PROVIDER_CONTRACT_V1,
};

/// Public world type used by gameplay providers without exposing scheduler internals.
pub type GameplayWorld = World;

/// Stable gameplay frame DTO passed to profile-owned providers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameplayFrame {
    pub dt: f32,
    pub fixed_tick: u64,
}

impl From<SimFrame> for GameplayFrame {
    #[inline]
    fn from(frame: SimFrame) -> Self {
        Self {
            dt: frame.dt,
            fixed_tick: frame.fixed_tick,
        }
    }
}

/// Stable gameplay execution phases exposed by the reusable runtime.
///
/// The engine owns phase ordering only. Concrete gameplay packages are selected
/// by the active application/profile and register providers explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameplayExecutionPhase {
    BeforePhysics,
    AfterPhysics,
    AfterDerived,
}

/// Profile-owned gameplay execution provider.
///
/// Providers may dispatch one or many systems internally. The important boundary
/// is that the reusable engine loop no longer names FPS, inventory, combat,
/// projectiles, missions, or any other product-specific gameplay system.
pub trait GameplaySystemProvider: Send + Sync {
    fn id(&self) -> &'static str;

    #[inline]
    fn descriptor(&self) -> RuntimeProviderDescriptor {
        RuntimeProviderDescriptor::gameplay_system(self.id())
    }

    fn run_phase(
        &self,
        phase: GameplayExecutionPhase,
        world: &mut GameplayWorld,
        frame: GameplayFrame,
    );
}

/// Ordered registry of gameplay providers selected by the active runtime profile.
///
/// Registration is idempotent by provider id: registering a replacement with the
/// same id updates the provider in-place and preserves deterministic order.
#[derive(Default)]
pub struct GameplaySystemProviderRegistry {
    providers: Vec<Arc<dyn GameplaySystemProvider>>,
}

impl GameplaySystemProviderRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(&mut self, provider: Arc<dyn GameplaySystemProvider>) {
        let descriptor = provider.descriptor();
        if let Err(error) = validate_provider_contract(
            descriptor,
            I_GAMEPLAY_SYSTEM_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
        ) {
            newengine_ulog_api::ulog::warn!("gameplay system provider rejected: {}", error);
            return;
        }
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id() == provider.id())
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

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn run_phase(
        &self,
        phase: GameplayExecutionPhase,
        world: &mut GameplayWorld,
        frame: GameplayFrame,
    ) {
        for provider in &self.providers {
            provider.run_phase(phase, world, frame);
        }
    }
}
