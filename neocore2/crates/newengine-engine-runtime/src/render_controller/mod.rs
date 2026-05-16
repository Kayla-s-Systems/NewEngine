#![forbid(unsafe_op_in_unsafe_fn)]

mod controller;
mod gpu;
mod material_bindings;
mod metrics;
mod module_impl;
mod resource_lifetime;
mod state;
mod resource_cache;
mod render_quality;
mod viewport;

pub use controller::RuntimeRenderController;

/// Public in-process extension points for profile-owned render feature packs.
///
/// These traits are not renderer backend APIs. They let a profile crate register
/// draw-list extraction and light/shadow planning policy while the renderer
/// backend remains replaceable behind `render.api`.
pub mod feature_api {
    pub use super::module_impl::draw_lists::{
        shadow_and_opaque_list, ui_list, DrawListBuildCtx, RenderDrawListProvider,
        RenderDrawListProviderMetadata, SceneExtractionCtx,
    };
    pub use super::module_impl::light_extraction::{
        LightExtractionCtx, LightExtractionProvider, LightExtractionProviderMetadata,
    };
    pub use super::module_impl::lights::{
        primary_directional_light, primary_point_light, PackedLights,
    };
    pub use super::module_impl::passes::{
        draw_primitives, draw_primitives_shadow, draw_procedural_terrain,
        draw_procedural_terrain_shadow,
    };
    pub use super::module_impl::scene::BoundsSnap;
    pub use super::module_impl::shadows::{
        retire_shadow_rt, try_build_directional_shadow_plan,
        warn_unsupported_point_shadow_once, warn_unsupported_spot_shadow_once,
        LightShadowPlan, ShadowFrame, ShadowLightKind,
    };
}
pub use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
