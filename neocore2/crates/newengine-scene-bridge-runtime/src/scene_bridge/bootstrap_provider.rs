#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;
use newengine_scene::Scene;

use newengine_runtime_provider_api::RuntimeProviderDescriptor;

/// Mutable scene-assembly surface exposed to application/profile bootstrap providers.
/// The provider receives only scene composition registries; host lifecycle, authority,
/// selection and Play activation remain owned by `SceneBridge`.
pub struct SceneBootstrapContext<'a> {
    pub scene: &'a mut Scene,
    pub primitives: &'a mut PrimitiveRegistry,
    pub materials: &'a MaterialRegistry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneBootstrapResult {
    pub primary_entity: Option<EntityId>,
}

impl SceneBootstrapResult {
    #[inline]
    pub const fn new(primary_entity: Option<EntityId>) -> Self {
        Self { primary_entity }
    }
}

/// Application-owned scene assembly contract.
///
/// Generic engine/runtime code dispatches this provider without knowing whether the
/// application is an FPS, RTS, editor preview, benchmark or another product profile.
pub trait SceneBootstrapProvider: Send + Sync {
    fn id(&self) -> &'static str;

    #[inline]
    fn descriptor(&self) -> RuntimeProviderDescriptor {
        RuntimeProviderDescriptor::scene_bootstrap(self.id())
    }

    fn bootstrap(
        &self,
        ctx: &mut SceneBootstrapContext<'_>,
    ) -> Result<SceneBootstrapResult, String>;
}
