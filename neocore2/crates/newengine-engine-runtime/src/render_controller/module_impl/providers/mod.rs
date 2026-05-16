#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use super::draw_lists::{RenderDrawListProvider, RenderDrawListProviderRegistry};

mod primitive_mesh_provider;
mod terrain_provider;
mod ui_provider;

use primitive_mesh_provider::PrimitiveMeshProvider;
use terrain_provider::TerrainProvider;
use ui_provider::UiProvider;

#[inline]
pub(super) fn standard_runtime_draw_list_provider_registry() -> RenderDrawListProviderRegistry {
    let mut registry = RenderDrawListProviderRegistry::new();
    register_builtin(&mut registry, TerrainProvider);
    register_builtin(&mut registry, PrimitiveMeshProvider);
    register_builtin(&mut registry, UiProvider);
    registry
}

#[inline]
fn register_builtin<T>(registry: &mut RenderDrawListProviderRegistry, provider: T)
where
    T: RenderDrawListProvider + 'static,
{
    registry.register_provider(Arc::new(provider));
}
