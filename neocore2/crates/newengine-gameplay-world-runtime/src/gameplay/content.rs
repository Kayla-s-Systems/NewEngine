#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::execution::GameplayWorld;
use newengine_runtime_provider_api::{
    validate_provider_contract, RuntimeProviderDescriptor, I_GAMEPLAY_CONTENT_PROVIDER_V1,
    PROVIDER_CONTRACT_V1,
};

/// Profile-owned gameplay content installer.
///
/// The reusable engine owns only the installation boundary. Concrete item catalogs,
/// loadouts, archetypes and authored package policy belong to gameplay/profile crates.
pub trait GameplayContentProvider: Send + Sync {
    fn id(&self) -> &'static str;

    #[inline]
    fn descriptor(&self) -> RuntimeProviderDescriptor {
        RuntimeProviderDescriptor::gameplay_content(self.id())
    }

    fn install(&self, world: &mut GameplayWorld) -> Result<(), String>;

    /// Reports whether the provider's installed content is still present in the current world.
    /// Providers with snapshot-sensitive resources should override this so a Play/Stop restore
    /// or scene replacement can trigger deterministic re-installation.
    #[inline]
    fn content_is_present(&self, _world: &GameplayWorld) -> bool {
        true
    }
}

#[derive(Default)]
pub struct GameplayContentProviderRegistry {
    providers: Vec<Arc<dyn GameplayContentProvider>>,
    installed: BTreeSet<&'static str>,
    failed: BTreeMap<&'static str, String>,
}

impl GameplayContentProviderRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(&mut self, provider: Arc<dyn GameplayContentProvider>) {
        let descriptor = provider.descriptor();
        if let Err(error) = validate_provider_contract(
            descriptor,
            I_GAMEPLAY_CONTENT_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
        ) {
            newengine_ulog_api::ulog::warn!("gameplay content provider rejected: {}", error);
            return;
        }
        let id = descriptor.id;
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id() == id)
        {
            *existing = provider;
            self.installed.remove(id);
            self.failed.remove(id);
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

    /// Installs providers when first registered or when their content disappeared from the world.
    ///
    /// Failures are explicit and sticky rather than silently falling back to product defaults
    /// inside engine code. Re-registering a provider resets its failure/install state.
    pub fn install_pending(&mut self, world: &mut GameplayWorld) {
        let pending = self
            .providers
            .iter()
            .filter(|provider| {
                let id = provider.id();
                !self.failed.contains_key(id)
                    && (!self.installed.contains(id) || !provider.content_is_present(world))
            })
            .cloned()
            .collect::<Vec<_>>();

        for provider in pending {
            let id = provider.id();
            match provider.install(world) {
                Ok(()) => {
                    self.installed.insert(id);
                    newengine_ulog_api::ulog::info!(
                        "gameplay content provider installed id='{}'",
                        id
                    );
                }
                Err(error) => {
                    self.installed.remove(id);
                    newengine_ulog_api::ulog::warn!(
                        "gameplay content provider failed id='{}' err='{}' policy='no-hidden-fallback'",
                        id,
                        error
                    );
                    self.failed.insert(id, error);
                }
            }
        }
    }

    #[inline]
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.contains(id)
    }

    #[inline]
    pub fn failure(&self, id: &str) -> Option<&str> {
        self.failed.get(id).map(String::as_str)
    }
}
