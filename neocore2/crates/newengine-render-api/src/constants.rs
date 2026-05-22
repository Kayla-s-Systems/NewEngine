/// Engine-facing render service gateway id. Consumers call this facade; the host
/// resolves it to the active renderer provider service by descriptor metadata.
pub const ENGINE_RENDER_SERVICE_ID: &str = "engine.render";
/// Third-level render domain for post-process/effect stack providers.
pub const ENGINE_RENDER_EFFECTS_SERVICE_ID: &str = "engine.render.effects";
/// Third-level render domain for material system providers.
pub const ENGINE_RENDER_MATERIALS_SERVICE_ID: &str = "engine.render.materials";

/// Default/first-party provider service id for render backends.
pub const RENDER_SERVICE_ID: &str = "render.api";
pub const RENDER_BACKEND_CAPABILITY_ID: &str = "render.backend";
pub const RENDER_EFFECTS_SERVICE_ID: &str = "render.effects.api";
pub const RENDER_EFFECTS_BACKEND_CAPABILITY_ID: &str = "render.effects.backend";
pub const RENDER_MATERIALS_SERVICE_ID: &str = "render.materials.api";
pub const RENDER_MATERIALS_BACKEND_CAPABILITY_ID: &str = "render.materials.backend";
pub const RENDER_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const RENDER_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const RENDER_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
/// Binary hot-path method for frame-local unit render commands.
///
/// The regular invoke_json surface remains the typed control protocol. This
/// method is intentionally narrow: it carries only unit commands such as
/// write_buffer/set_pipeline/draw so draw-list extraction does not serialize
/// byte payloads as JSON arrays on every frame.
pub const RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V1: &str = "command_batch_bin_v1";

/// Renderer diagnostics surfaces. These are JSON control-plane dumps, not frame hot-path commands.
pub const RENDER_SERVICE_METHOD_DUMP_PHASE_GRAPH_V1: &str = "engine.render.dump_phase_graph_v1";
pub const RENDER_SERVICE_METHOD_DUMP_RESOURCE_LIFETIME_V1: &str = "engine.render.dump_resource_lifetime_v1";
pub const RENDER_SERVICE_METHOD_DUMP_SHADER_CACHE_V1: &str = "engine.render.dump_shader_cache_v1";
pub const RENDER_SERVICE_METHOD_DUMP_EFFECT_GRAPH_V1: &str = "engine.render.dump_effect_graph_v1";

/// Generic backend-family declaration for render providers.
pub const RENDER_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "render",
        ENGINE_RENDER_SERVICE_ID,
        RENDER_SERVICE_ID,
        RENDER_BACKEND_CAPABILITY_ID,
    );

pub const RENDER_EFFECTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "render.effects",
        ENGINE_RENDER_EFFECTS_SERVICE_ID,
        RENDER_EFFECTS_SERVICE_ID,
        RENDER_EFFECTS_BACKEND_CAPABILITY_ID,
    );

pub const RENDER_MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "render.materials",
        ENGINE_RENDER_MATERIALS_SERVICE_ID,
        RENDER_MATERIALS_SERVICE_ID,
        RENDER_MATERIALS_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing render gateway.
pub const RENDER_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_RENDER_SERVICE_ID,
        "newengine.render-api >= 0.3.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for render. Missing render degrades unless
/// the explicit env switch is enabled by a strict test/runtime profile.
pub const RENDER_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        RENDER_RUNTIME_CONTRACT_SPEC,
        Some(RENDER_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_RENDER_BACKEND"),
    );

pub type Color4 = [f32; 4];
pub type RenderWireResult<T> = Result<T, String>;
