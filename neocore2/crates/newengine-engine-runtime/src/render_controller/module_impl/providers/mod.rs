#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use super::draw_lists::{RenderDrawListProvider, RenderDrawListProviderRegistry};

mod collision_debug_provider;
mod grid_provider;
mod light_gizmo_provider;
mod primitive_mesh_provider;
mod terrain_provider;
mod ui_provider;

use collision_debug_provider::CollisionDebugProvider;
use grid_provider::GridProvider;
use light_gizmo_provider::LightGizmoProvider;
use primitive_mesh_provider::PrimitiveMeshProvider;
use terrain_provider::TerrainProvider;
use ui_provider::UiProvider;

#[inline]
pub(super) fn standard_runtime_draw_list_provider_registry() -> RenderDrawListProviderRegistry {
    let mut registry = RenderDrawListProviderRegistry::new();
    register_builtin(&mut registry, TerrainProvider);
    register_builtin(&mut registry, PrimitiveMeshProvider);
    register_builtin(&mut registry, GridProvider);
    register_builtin(&mut registry, LightGizmoProvider);
    register_builtin(&mut registry, CollisionDebugProvider);
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
