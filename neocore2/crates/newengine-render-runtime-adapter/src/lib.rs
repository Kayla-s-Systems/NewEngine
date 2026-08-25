#![forbid(unsafe_op_in_unsafe_fn)]

mod client;
mod runtime_module;
mod service_api;
mod types;

pub use runtime_module::RenderBackendRuntimeModule;
pub use types::ResolvedRenderBackendConfig;

/// Backend-neutral implementation unit consumed by the generic runtime-unit catalog.
///
/// `render.backend` is the logical capability that activates this bridge. The unit itself
/// provides the in-process runtime RenderApi and binds whatever provider was selected by
/// the immutable CompositionPlan; it never selects Vulkan/D3D12/Metal/etc. by name.
pub const RENDER_RUNTIME_UNIT_ID: &str = "engine.runtime-adapter.render";
pub const RENDER_RUNTIME_UNIT_SPEC: newengine_service_api::EngineRuntimeUnitSpec =
    newengine_service_api::EngineRuntimeUnitSpec::new(
        RENDER_RUNTIME_UNIT_ID,
        1,
        newengine_service_api::EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.render-api"],
        &[newengine_render_api::RENDER_BACKEND_CAPABILITY_ID],
        &["engine.runtime-unit", "backend-neutral", "service-adapter"],
    );
