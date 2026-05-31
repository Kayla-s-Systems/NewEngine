// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

/// Engine-facing UI service gateway id. Runtime consumers call this facade;
/// the host resolves it to the active UI provider by descriptor metadata.
pub const ENGINE_UI_SERVICE_ID: &str = "engine.ui";

/// Default/first-party provider service id for UI backends.
pub const UI_SERVICE_ID: &str = "ui.api";
pub const UI_BACKEND_CAPABILITY_ID: &str = "ui.backend";

pub const UI_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const UI_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const UI_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const UI_SERVICE_METHOD_SURFACE_MANIFEST_V1: &str = "surface_manifest_v1";
pub const UI_SERVICE_METHOD_SURFACE_CATALOG_V1: &str = "surface_catalog_v1";
pub const UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1: &str = "layout_manifest_v1";
pub const UI_SERVICE_METHOD_ACTION_MANIFEST_V1: &str = "action_manifest_v1";
pub const UI_SERVICE_METHOD_LOADING_SHELL_V1: &str = "loading_shell_v1";
pub const UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA: &str = "debug_telemetry_schema";
/// Generic retained UI surface/node state. Runtime publishes state only; provider owns layout/rendering.
pub const UI_SERVICE_METHOD_SURFACE_NODE_V1: &str = "ui.surface_node_v1";
pub const UI_SERVICE_METHOD_DRAW_FRAME_V1: &str = "draw_frame_v1";
pub const UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1: &str = "draw_frame_bin_v1";

/// Provider-owned XML document export for diagnostics/import tooling only.
/// Runtime UI should mount compiled `.neui` documents through the methods below.
pub const UI_SERVICE_METHOD_DOCUMENT_XML_V1: &str = "document_xml_v1";

/// Canonical live runtime UI methods. `engine.assets.ui` compiles `.neui` entries;
/// `engine.ui` mounts, patches, routes input/actions and emits draw packets.
pub const UI_SERVICE_METHOD_REGISTRY_LOAD_V1: &str = "ui.registry_load_v1";
pub const UI_SERVICE_METHOD_MOUNT_SURFACE_V1: &str = "ui.mount_surface_v1";
/// Provider-neutral runtime node creation request.
///
/// This is the generative sibling of `.neui` mounting: tools, plugins and
/// scripts can submit a typed UI node tree and the active provider receives the
/// same retained `UiSurfaceNode` contract as authored documents.
pub const UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1: &str = "ui.apply_node_request_v1";
pub const UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1: &str = "ui.unmount_surface_v1";
pub const UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1: &str = "ui.set_surface_visible_v1";
pub const UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1: &str = "ui.apply_state_patch_v1";
pub const UI_SERVICE_METHOD_DISPATCH_INPUT_V1: &str = "ui.dispatch_input_v1";
pub const UI_SERVICE_METHOD_DISPATCH_ACTION_V1: &str = "ui.dispatch_action_v1";
pub const UI_SERVICE_METHOD_NAVIGATE_V1: &str = "ui.navigate_v1";
pub const UI_SERVICE_METHOD_DEBUG_TREE_V1: &str = "ui.debug_tree_v1";
pub const UI_SERVICE_METHOD_DEBUG_BINDINGS_V1: &str = "ui.debug_bindings_v1";

pub const UI_SURFACE_ENGINE_LOADING: &str = "engine.ui.loading";
pub const UI_SURFACE_ENGINE_ERROR_MODAL: &str = "engine.error_modal";
pub const UI_SURFACE_RUNTIME_OVERLAY: &str = "runtime.overlay";
pub const UI_SURFACE_RUNTIME_DEBUG_OVERLAY: &str = "runtime.debug_overlay";
pub const UI_SURFACE_ENGINE_PRIMARY: &str = "engine.ui.primary";
/// Canonical declarative UI theme id used by first-party runtime/editor surfaces.
/// The engine treats this as a token; the active UI provider resolves fonts,
/// metrics and colors from its own theme registry.
pub const UI_THEME_NORTHSTAR_DEFAULT: &str = "northstar.default";
/// Compact editor theme token. Providers resolve the actual palette, metrics and
/// interaction states through theme assets/registry, not ad hoc draw branches.
pub const UI_THEME_NORTHSTAR_EDITOR: &str = "northstar.editor.light";
pub const UI_THEME_ASSET_NORTHSTAR_EDITOR: &str = "ui/themes/northstar_editor.neui@editor_light";

/// Canonical font asset references used by first-party editor surfaces.
///
/// `.yft` belongs to the NEF8/ListFile family and describes font families,
/// faces, source files and atlas policy. The engine stores references here;
/// concrete font binaries are imported by tooling and must not be hardcoded in
/// the UI provider.
pub const UI_FONT_ASSET_EDITOR_SANS: &str = "ui/fonts/editor.yft@inter_variable";
pub const UI_FONT_ASSET_EDITOR_DISPLAY: &str = "ui/fonts/editor.yft@granic_slab_medium";
pub const UI_FONT_ASSET_EDITOR_BOLD: &str = "ui/fonts/editor.yft@granic_sans_bold";
pub const UI_FONT_ASSET_BRAND: &str = "ui/fonts/editor.yft@pricedown_display";

/// Generic component primitives. These are not screen types: every interface is
/// the same retained `UiSurfaceNode` tree and may compose the same primitives.
pub const UI_COMPONENT_SURFACE: &str = "surface";
pub const UI_COMPONENT_PANEL: &str = "panel";
pub const UI_COMPONENT_STACK: &str = "stack";
pub const UI_COMPONENT_ROW: &str = "row";
pub const UI_COMPONENT_TEXT: &str = "text";
pub const UI_COMPONENT_ACTION: &str = "action";
pub const UI_COMPONENT_SPACER: &str = "spacer";
pub const UI_COMPONENT_COLUMN: &str = "column";
pub const UI_COMPONENT_GRID: &str = "grid";
pub const UI_COMPONENT_BUTTON: &str = "button";
pub const UI_COMPONENT_INPUT: &str = "input";
pub const UI_COMPONENT_CHECKBOX: &str = "checkbox";
pub const UI_COMPONENT_TOGGLE: &str = "toggle";
pub const UI_COMPONENT_SLIDER: &str = "slider";
pub const UI_COMPONENT_SCROLL_BAR: &str = "scroll_bar";
pub const UI_COMPONENT_SELECT: &str = "select";
pub const UI_COMPONENT_SEPARATOR: &str = "separator";
pub const UI_COMPONENT_LIST: &str = "list";
pub const UI_COMPONENT_TREE: &str = "tree";
pub const UI_COMPONENT_SPLIT: &str = "split";
pub const UI_COMPONENT_VIEWPORT: &str = "viewport";
pub const UI_COMPONENT_EXTERNAL_TEXTURE: &str = "external_texture";



/// Generic backend-family declaration for UI providers.
pub const UI_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ui",
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_ID,
        UI_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing UI gateway.
pub const UI_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_UI_SERVICE_ID,
        "newengine.ui-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for UI. Missing UI degrades unless strict
/// runtime profiles opt in through the explicit env switch.
pub const UI_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        UI_RUNTIME_CONTRACT_SPEC,
        Some(UI_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_UI_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub surfaces: Vec<String>,
}

impl Default for UiServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.ui-api/v1".to_owned(),
            features: vec![
                "provider-owned-layout".to_owned(),
                "declarative-actions".to_owned(),
                "runtime-debug-node-projection".to_owned(),
                "surface-node-modal-v1".to_owned(),
                "surface-node-navigation-v1".to_owned(),
                "surface-node-action-routes-v1".to_owned(),
                "draw-frame-bin-v1".to_owned(),
                "atlas-text-quads".to_owned(),
                "ui-font-style-tokens-v1".to_owned(),
                "ui-theme-font-tokens-v1".to_owned(),
                "ui-component-catalog-v1".to_owned(),
                "retained-component-node-tree-v1".to_owned(),
                "runtime-ui-node-request-v1".to_owned(),
                "generative-ui-tree-v1".to_owned(),
                "same-foundation-ui-node-v1".to_owned(),
                "theme-font-token-text-v1".to_owned(),
                "neui-compiled-document-mount-v1".to_owned(),
                "neui-schema-validation-v1".to_owned(),
                "neui-component-libraries-v1".to_owned(),
                "neui-theme-libraries-v1".to_owned(),
                "neui-component-expansion-v1".to_owned(),
                "binding-graph-execution-v1".to_owned(),
                "state-patch-bindings-v1".to_owned(),
                "retained-interaction-dispatcher-v1".to_owned(),
                "ui-dispatch-input-v1".to_owned(),
                "ui-layout-box-v1".to_owned(),
                "retained-layout-solver-v1".to_owned(),
                "shared-layout-hit-test-paint-v1".to_owned(),
                "retained-surface-node-v1".to_owned(),
                "debug-tree-v1".to_owned(),
                "debug-tree-layout-state-v2".to_owned(),
                "ui-devtools-overlays-v1".to_owned(),
                "layout-box-overlay-v1".to_owned(),
                "hit-test-overlay-v1".to_owned(),
                "focus-chain-viewer-v1".to_owned(),
                "z-order-top-layer-viewer-v1".to_owned(),
                "style-cascade-inspector-v1".to_owned(),
                "binding-graph-inspector-v1".to_owned(),
                "action-dispatch-log-v1".to_owned(),
                "input-capture-debugger-v1".to_owned(),
                "draw-cost-counter-v1".to_owned(),
                "texture-atlas-viewer-v1".to_owned(),
                "debug-bindings-v1".to_owned(),
            ],
            methods: ui_service_methods().iter().map(|it| (*it).to_owned()).collect(),
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
                UI_SURFACE_RUNTIME_OVERLAY.to_owned(),
                UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
                UI_SURFACE_ENGINE_PRIMARY.to_owned(),
            ],
        }
    }
}

pub const UI_SERVICE_METHODS: &[&str] = &[
    UI_SERVICE_METHOD_INFO,
    UI_SERVICE_METHOD_INVOKE,
    UI_SERVICE_METHOD_SHUTDOWN_V1,
    UI_SERVICE_METHOD_SURFACE_MANIFEST_V1,
    UI_SERVICE_METHOD_SURFACE_CATALOG_V1,
    UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1,
    UI_SERVICE_METHOD_ACTION_MANIFEST_V1,
    UI_SERVICE_METHOD_LOADING_SHELL_V1,
    UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA,
    UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DOCUMENT_XML_V1,
    UI_SERVICE_METHOD_REGISTRY_LOAD_V1,
    UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
    UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
    UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1,
    UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1,
    UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
    UI_SERVICE_METHOD_DISPATCH_INPUT_V1,
    UI_SERVICE_METHOD_DISPATCH_ACTION_V1,
    UI_SERVICE_METHOD_NAVIGATE_V1,
    UI_SERVICE_METHOD_DEBUG_TREE_V1,
    UI_SERVICE_METHOD_DEBUG_BINDINGS_V1,
];

#[inline]
pub const fn ui_service_methods() -> &'static [&'static str] {
    UI_SERVICE_METHODS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiInvokeEnvelope {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFrameRequest {
    pub version: u32,
    pub frame_index: u64,
    pub dt_sec: f32,
    pub surface_size_px: [u32; 2],
    pub pixels_per_point: f32,
}

impl UiFrameRequest {
    #[inline]
    pub fn new(frame_index: u64, dt_sec: f32, surface_size_px: [u32; 2], pixels_per_point: f32) -> Self {
        Self {
            version: 1,
            frame_index,
            dt_sec,
            surface_size_px,
            pixels_per_point: pixels_per_point.max(0.0001),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFrameResponse {
    pub version: u32,
    pub draw_list: UiDrawList,
}

impl UiFrameResponse {
    #[inline]
    pub fn new(draw_list: UiDrawList) -> Self {
        Self { version: 1, draw_list }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAck {
    pub ok: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl UiAck {
    #[inline]
    pub fn ok(provider: impl Into<String>) -> Self {
        Self { ok: true, provider: Some(provider.into()), message: None }
    }
}
