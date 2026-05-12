#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use super::light_extraction::{LightExtractionProvider, LightExtractionProviderRegistry};

mod ambient_occlusion_provider;
mod directional_shadow_provider;
mod point_cube_shadow_provider;
mod spot_shadow_provider;

use ambient_occlusion_provider::AmbientOcclusionProvider;
use directional_shadow_provider::DirectionalShadowProvider;
use point_cube_shadow_provider::PointCubeShadowProvider;
use spot_shadow_provider::SpotShadowProvider;

#[inline]
pub(super) fn standard_runtime_light_extraction_provider_registry() -> LightExtractionProviderRegistry {
    let mut registry = LightExtractionProviderRegistry::new();
    register_builtin(&mut registry, DirectionalShadowProvider);
    register_builtin(&mut registry, PointCubeShadowProvider);
    register_builtin(&mut registry, SpotShadowProvider);
    register_builtin(&mut registry, AmbientOcclusionProvider);
    registry
}

#[inline]
fn register_builtin<T>(registry: &mut LightExtractionProviderRegistry, provider: T)
where
    T: LightExtractionProvider + 'static,
{
    registry.register_provider(Arc::new(provider));
}
