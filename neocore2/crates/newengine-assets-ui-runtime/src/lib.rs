#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.ui` semantic service.

//!

//! `.neui` is a NEF8/ListFile UI dictionary. This crate owns the UI-domain

//! meaning of that dictionary: XMLcentral validation, surface/document selection,

//! dependency extraction and runtime DTO compilation. Consumers only call the

//! `engine.assets.ui` gateway and receive a response DTO.

use abi_stable::std_types::{RResult, RString};

use flate2::read::DeflateDecoder;

use newengine_assets_api::AssetServiceClient;

use newengine_assets_api::{
    assets_ui_method, list_file_content_kind_label as content_kind_label,
    parse_list_file_header_v1, ASSETS_UI_BACKEND_CAPABILITY_ID, ASSETS_UI_RUNTIME_CONTRACT,
    ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS, ENGINE_ASSETS_UI_SERVICE_ID,
    ENGINE_ASSET_SERVICE_ID, LIST_FILE_CONTENT_KIND_NEUI,
};

use newengine_plugin_api::Blob;

use newengine_service_api::EngineServiceKind;

use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};

use newengine_ui_api::{
    UiActionEdge, UiBindingEdge, UiBindingMode, UiBindingPlan, UiCompiledDocument,
    UiComponentLibraryRef, UiComponentTemplate, UiDocumentSource, UiDocumentSourceKind,
    UiNodeBindingRequest, UiNodeEventRoute, UiNodeEventTrigger, UiNodeRequest, UiNodeTone,
    UiRuntimeNodeKind, UiSourceSpan, UiStateSource, UiThemeLibraryRef, UiUpdatePolicy,
    UI_COMPONENT_ACTION, UI_COMPONENT_BUTTON, UI_COMPONENT_CHECKBOX, UI_COMPONENT_EXTERNAL_TEXTURE,
    UI_COMPONENT_GRID, UI_COMPONENT_INPUT, UI_COMPONENT_LIST, UI_COMPONENT_ROW,
    UI_COMPONENT_SCROLL_BAR, UI_COMPONENT_SELECT, UI_COMPONENT_SEPARATOR, UI_COMPONENT_SLIDER,
    UI_COMPONENT_SPACER, UI_COMPONENT_STACK, UI_COMPONENT_SURFACE, UI_COMPONENT_TEXT,
    UI_COMPONENT_TOGGLE, UI_COMPONENT_TREE, UI_COMPONENT_VIEWPORT,
};

use newengine_ui_navigation_api::{
    UiNodeActionRoute, UiNodeFeedbackEvent, UiNodeFeedbackSeverity, UiNodeNavigationDocument,
    UiNodeNavigationItem, UiNodeNavigationPage, UiNodeNavigationTone, UiNodeTransition,
    UiNodeTransitionKind,
};

use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, HashMap};

use std::io::Read;

pub const ASSETS_UI_GATEWAY_OWNER: &str = "newengine-assets-ui-runtime.engine-runtime-provider";

mod compile_request;

mod navigation;

mod node_compile;

mod theme;

mod xml;

pub(crate) use navigation::*;

pub(crate) use node_compile::*;

pub(crate) use theme::*;

pub(crate) use xml::*;

pub struct AssetsUiRuntimeState {
    client: AssetServiceClient,

    xml_cache: HashMap<String, String>,

    compile_cache: HashMap<String, AssetsUiCompileResponse>,
}

impl AssetsUiRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,

            xml_cache: HashMap::new(),

            compile_cache: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AssetsUiServiceInfo {
    pub id: &'static str,

    pub gateway: &'static str,

    pub provider: &'static str,

    pub contract: &'static str,

    pub byte_owner: &'static str,

    pub semantic_owner: &'static str,

    pub runtime_owner: &'static str,

    pub methods: &'static [&'static str],

    pub policy: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiRefRequest {
    pub document_ref: String,

    pub ui_ref: String,

    pub logical_path: String,

    pub entry: String,
}

impl Default for AssetsUiRefRequest {
    #[inline]
    fn default() -> Self {
        Self {
            document_ref: String::new(),

            ui_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiCompileRequest {
    pub document_ref: String,

    pub ui_ref: String,

    pub logical_path: String,

    pub entry: String,

    pub style_ref: Option<String>,

    pub source_kind: UiDocumentSourceKind,

    pub stream_id: Option<String>,

    pub generator_id: Option<String>,

    pub mount_runtime: bool,
}

impl Default for AssetsUiCompileRequest {
    #[inline]
    fn default() -> Self {
        Self {
            document_ref: String::new(),

            ui_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),

            style_ref: None,

            source_kind: UiDocumentSourceKind::Asset,

            stream_id: None,

            generator_id: None,

            mount_runtime: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiCompileResponse {
    pub ok: bool,

    pub schema: String,

    pub document_ref: String,

    pub logical_path: String,

    pub vfs_path: String,

    pub entry: String,

    pub surface_id: String,

    pub xmlcentral: String,

    pub compiled_document: UiCompiledDocument,

    pub navigation_document: Option<UiNodeNavigationDocument>,

    pub source_kind: UiDocumentSourceKind,

    pub style_ref: Option<String>,

    pub dependencies: Vec<String>,

    pub style_dependencies: Vec<String>,

    pub warnings: Vec<String>,
}

impl Default for AssetsUiCompileResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,

            schema: "newengine.assets.ui.compile_document.response.v1".to_owned(),

            document_ref: String::new(),

            logical_path: String::new(),

            vfs_path: String::new(),

            entry: String::new(),

            surface_id: String::new(),

            xmlcentral: String::new(),

            compiled_document: UiCompiledDocument::default(),

            navigation_document: None,

            source_kind: UiDocumentSourceKind::Asset,

            style_ref: None,

            dependencies: Vec::new(),

            style_dependencies: Vec::new(),

            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(default)]
pub struct AssetsUiDiagnosticResponse {
    pub ok: bool,

    pub schema: String,

    pub document_ref: String,

    pub logical_path: String,

    pub entry: String,

    pub entry_id: String,

    pub source_span: UiSourceSpan,

    pub message: String,

    pub warnings: Vec<String>,
}

impl Default for AssetsUiDiagnosticResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,

            schema: "newengine.assets.ui.diagnostic.v1".to_owned(),

            document_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),

            entry_id: String::new(),

            source_span: UiSourceSpan::default(),

            message: String::new(),

            warnings: Vec::new(),
        }
    }
}

/// Compile a decoded NEUI XMLcentral string directly into a provider-neutral UI tree.
///
/// This helper is intentionally DTO-only: callers that already own bytes/asset
/// lifetime can avoid reimplementing XML parsing while still keeping engine.ui
/// presentation authored by `.neui` instead of Rust code.
pub fn compile_xmlcentral_surface_root(
    xml: &str,

    document_ref: &str,

    style_ref: Option<&str>,
) -> Result<UiNodeRequest, String> {
    compile_request::validate_requested_entry(xml, "surface")?;

    let surface = parse_surface(xml)
        .ok_or_else(|| format!("{document_ref}: .neui document has no <Surface> entry"))?;

    compile_surface_root(
        xml,
        &surface,
        document_ref,
        style_ref.or(surface.theme.as_deref()),
    )
}

/// Decode packed `.neui` NEF8 bytes and compile its `@surface` entry into a provider-neutral UI tree.
pub fn compile_neui_bytes_surface_root(
    bytes: &[u8],

    document_ref: &str,

    style_ref: Option<&str>,
) -> Result<UiNodeRequest, String> {
    let (logical_path, _) = compile_request::split_ref(document_ref);

    let xml = compile_request::decode_neui_xmlcentral(
        if logical_path.trim().is_empty() {
            document_ref
        } else {
            &logical_path
        },
        bytes,
    )?;

    compile_xmlcentral_surface_root(&xml, document_ref, style_ref)
}

pub fn assets_ui_service_info() -> AssetsUiServiceInfo {
    AssetsUiServiceInfo {

        id: ASSETS_UI_SERVICE_ID,

        gateway: ENGINE_ASSETS_UI_SERVICE_ID,

        provider: "StarVaultAssetsUiRuntimeProvider",

        contract: ASSETS_UI_RUNTIME_CONTRACT,

        byte_owner: ENGINE_ASSET_SERVICE_ID,

        semantic_owner: ENGINE_ASSETS_UI_SERVICE_ID,

        runtime_owner: newengine_ui_api::ENGINE_UI_SERVICE_ID,

        methods: ASSETS_UI_SERVICE_METHODS,

        policy: ".neui is a binary NEF8/ListFile envelope with no raw JSON metadata payload; engine.assets.ui owns semantic decode and consumers receive compiled DTOs",

    }
}

fn invoke_json(state: &mut AssetsUiRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,

        Err(e) => return RResult::RErr(RString::from(e)),
    };

    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(assets_ui_method::COMPILE_DOCUMENT_V1);

    let request_value = value.get("request").cloned().unwrap_or_default();

    match method {
        assets_ui_method::COMPILE_DOCUMENT_V1 => {
            let request =
                serde_json::from_value::<AssetsUiCompileRequest>(request_value).unwrap_or_default();

            match compile_request::compile_document(state, request.clone()) {
                Ok(response) => ok_json(response),

                Err(e) => ok_json(compile_request::error_response_from_compile_error(
                    e, &request,
                )),
            }
        }

        assets_ui_method::DOCUMENT_V1 | assets_ui_method::DUMP_XMLCENTRAL_V1 => {
            let request =
                serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();

            match compile_request::load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => ok_json(serde_json::json!({

                    "ok": true,

                    "schema": "newengine.assets.ui.document.response.v1",

                    "document_ref": resolved.document_ref,

                    "logical_path": resolved.logical_path,

                    "vfs_path": resolved.vfs_path,

                    "entry": resolved.entry,

                    "xmlcentral": xml,

                })),

                Err(e) => ok_json(compile_request::error_response_from_message(e)),
            }
        }

        assets_ui_method::VALIDATE_V1 => {
            let request =
                serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();

            match compile_request::load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => {
                    let source_span = source_span_for_offset(&xml, 0, &resolved.document_ref);

                    ok_json(AssetsUiDiagnosticResponse {
                        ok: true,

                        document_ref: resolved.document_ref,

                        logical_path: resolved.logical_path,

                        entry_id: resolved.entry.clone(),

                        entry: resolved.entry,

                        source_span,

                        message: format!(
                            "valid binary .neui decoded to XMLcentral bytes={} root={}",
                            xml.len(),
                            root_name(&xml).unwrap_or("unknown")
                        ),

                        ..Default::default()
                    })
                }

                Err(e) => ok_json(compile_request::error_response_from_message(e)),
            }
        }

        assets_ui_method::DEPENDENCIES_V1 => {
            let request =
                serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();

            match compile_request::load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => ok_json(serde_json::json!({

                    "ok": true,

                    "schema": "newengine.assets.ui.dependencies.response.v1",

                    "document_ref": resolved.document_ref,

                    "logical_path": resolved.logical_path,

                    "entry": resolved.entry,

                    "dependencies": extract_dependencies(&xml),

                })),

                Err(e) => ok_json(compile_request::error_response_from_message(e)),
            }
        }

        assets_ui_method::MANIFEST_V1
        | assets_ui_method::ENTRY_V1
        | assets_ui_method::REGISTRY_V1
        | assets_ui_method::BINDING_PLAN_V1 => {
            let request =
                serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();

            match compile_request::compile_document(
                state,
                compile_request::compile_request_from_ref(request),
            ) {
                Ok(response) => ok_json(response),

                Err(e) => ok_json(compile_request::error_response_from_message(e)),
            }
        }

        other => RResult::RErr(RString::from(format!(
            "engine.assets.ui: unknown invoke_json method '{other}'"
        ))),
    }
}

pub fn assets_ui_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(

        ASSETS_UI_SERVICE_ID,

        ASSETS_UI_GATEWAY_OWNER,

        ASSETS_UI_BACKEND_CAPABILITY_ID,

        ASSETS_UI_SERVICE_METHODS.iter().copied(),

    )

    .gateway(ENGINE_ASSETS_UI_SERVICE_ID)

    .protocol(ASSETS_UI_RUNTIME_CONTRACT)

    .features(["neui-nef8-binary-envelope", "neui-no-json-runtime-metadata", "compile-document-v1", "ui-node-navigation-dto", "dependency-extraction"])

    .notes("Engine UI asset semantic service. Consumers call engine.assets.ui and receive runtime DTOs; engine.ui owns only live mount/state/input/draw runtime.");

    JsonServiceRouter::with_state(ASSETS_UI_SERVICE_ID, AssetsUiRuntimeState::new(client))
        .describe_json(&description)
        .info(assets_ui_service_info)
        .post_json_result::<AssetsUiCompileRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::COMPILE_DOCUMENT_V1,
            compile_request::compile_document,
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DOCUMENT_V1,
            |state, request| {
                let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;

                Ok(serde_json::json!({

                    "ok": true,

                    "schema": "newengine.assets.ui.document.response.v1",

                    "document_ref": resolved.document_ref,

                    "logical_path": resolved.logical_path,

                    "vfs_path": resolved.vfs_path,

                    "entry": resolved.entry,

                    "xmlcentral": xml,

                }))
            },
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DUMP_XMLCENTRAL_V1,
            |state, request| {
                let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;

                Ok(serde_json::json!({

                    "ok": true,

                    "schema": "newengine.assets.ui.xmlcentral_dump.v1",

                    "document_ref": resolved.document_ref,

                    "logical_path": resolved.logical_path,

                    "vfs_path": resolved.vfs_path,

                    "entry": resolved.entry,

                    "xmlcentral": xml,

                }))
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiDiagnosticResponse, _>(
            assets_ui_method::VALIDATE_V1,
            |state, request| {
                let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;

                Ok(AssetsUiDiagnosticResponse {
                    ok: true,

                    document_ref: resolved.document_ref,

                    logical_path: resolved.logical_path,

                    entry: resolved.entry,

                    message: format!(
                        "valid binary .neui decoded to XMLcentral bytes={} root={}",
                        xml.len(),
                        root_name(&xml).unwrap_or("unknown")
                    ),

                    ..Default::default()
                })
            },
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DEPENDENCIES_V1,
            |state, request| {
                let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;

                Ok(serde_json::json!({

                    "ok": true,

                    "schema": "newengine.assets.ui.dependencies.response.v1",

                    "document_ref": resolved.document_ref,

                    "logical_path": resolved.logical_path,

                    "entry": resolved.entry,

                    "dependencies": extract_dependencies(&xml),

                }))
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::MANIFEST_V1,
            |state, request| {
                compile_request::compile_document(
                    state,
                    compile_request::compile_request_from_ref(request),
                )
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::ENTRY_V1,
            |state, request| {
                compile_request::compile_document(
                    state,
                    compile_request::compile_request_from_ref(request),
                )
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::REGISTRY_V1,
            |state, request| {
                compile_request::compile_document(
                    state,
                    compile_request::compile_request_from_ref(request),
                )
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::BINDING_PLAN_V1,
            |state, request| {
                compile_request::compile_document(
                    state,
                    compile_request::compile_request_from_ref(request),
                )
            },
        )
        .blob(assets_ui_method::INVOKE_JSON, invoke_json)
        .blob(assets_ui_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_assets_ui_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_UI_SERVICE_ID,

        service_kind: EngineServiceKind::AssetUi,

        provider_service: ASSETS_UI_SERVICE_ID,

        provider_route: "engine.assets.starvault.ui",

        capability: ASSETS_UI_BACKEND_CAPABILITY_ID,

        priority: 0,

        owner: ASSETS_UI_GATEWAY_OWNER,

        service: assets_ui_gateway_service(client),
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::compile_request::validate_requested_entry;

    #[test]
    fn parses_navigation_document_from_xmlcentral() {
        let xml = r#"<NeUiDictionary><UiNodeNavigationDocument id="engine.ui.primary" surface_id="engine.ui.primary" root_page="root" title="UI"><Page id="root"><Item id="resume" label="Resume"><Action id="resume" source="s" target="UiNodeNavigationRuntime" event="ui.close" transition="close" /></Item></Page></UiNodeNavigationDocument></NeUiDictionary>"#;

        let doc = parse_navigation_document(xml).unwrap().unwrap();

        assert_eq!(doc.id, "engine.ui.primary");

        assert_eq!(doc.pages[0].items[0].label, "Resume");
    }

    #[test]
    fn derives_navigation_document_from_surface_layout_buttons() {
        let xml = r#"

<NeUiDictionary document_kind="surface">

  <Surface name="engine.main_menu" kind="main_menu" modal="true" z_order="700" root="layout.main" />

  <Layout name="layout.main" surface="engine.main_menu">

    <Panel id="main.root" class="menu-shell">

      <Text id="main.title" class="title" value="NewEngine" />

      <Button id="main.start" class="button button-primary"><Text value="Start" /><Event trigger="click" action="game.start" /></Button>

      <Button id="main.settings" class="button button-secondary"><Text value="Settings" /><Event trigger="click" action="engine.settings.open" /></Button>

    </Panel>

  </Layout>

  <ActionMap name="actions">

    <Action id="engine.settings.open" target="engine.ui.navigation" command="menu.open_page"><Payload page="settings" /></Action>

  </ActionMap>

</NeUiDictionary>

"#;

        let surface = SurfaceInfo {
            name: "engine.main_menu".to_owned(),

            kind: "main_menu".to_owned(),

            root: "layout.main".to_owned(),

            theme: None,

            modal: true,

            z_order: 700,
        };

        let doc = derive_navigation_document_from_surface_layout(xml, &surface)
            .unwrap()
            .unwrap();

        assert_eq!(doc.surface_id, "engine.main_menu");

        assert_eq!(doc.pages[0].items[0].label, "Start");

        assert_eq!(doc.pages[0].items[1].label, "Settings");
    }

    #[test]
    fn compiles_neui_surface_layout_into_root_node_request() {
        let xml = r#"

<NeUiDictionary document_kind="surface">

  <Surface name="engine.main_menu" kind="main_menu" modal="true" z_order="700" root="layout.main" theme="assets/ui/themes/northstar_editor.neui@editor_light" />

  <Layout name="layout.main" surface="engine.main_menu">

    <Panel id="main.root" class="menu-shell">

      <Text id="main.title" class="title" value="NewEngine" />

      <Button id="main.start" class="button button-primary"><Text value="Start" /><Event trigger="click" action="game.start" /></Button>

      <Select id="graphics.quality"><Option id="graphics.high" label="High" value="high" /></Select>

    </Panel>

  </Layout>

</NeUiDictionary>

"#;

        let surface = SurfaceInfo {
            name: "engine.main_menu".to_owned(),

            kind: "main_menu".to_owned(),

            root: "layout.main".to_owned(),

            theme: Some("assets/ui/themes/northstar_editor.neui@editor_light".to_owned()),

            modal: true,

            z_order: 700,
        };

        let root = compile_surface_root(
            xml,
            &surface,
            "assets/ui/engine/main_menu.neui@surface",
            surface.theme.as_deref(),
        )
        .unwrap();

        assert_eq!(root.kind, UiRuntimeNodeKind::Surface);

        assert_eq!(root.children.len(), 1);

        let panel = &root.children[0];

        assert_eq!(panel.id, "main.root");

        let button = panel
            .children
            .iter()
            .find(|node| node.id == "main.start")
            .unwrap();

        assert_eq!(button.action_id.as_deref(), Some("game.start"));

        assert!(button.interactive);

        let select = panel
            .children
            .iter()
            .find(|node| node.id == "graphics.quality")
            .unwrap();

        assert!(select.interactive);

        assert_eq!(
            select.children[0].action_id.as_deref(),
            Some("ui.select.high")
        );
    }

    fn find_node<'a>(node: &'a UiNodeRequest, id: &str) -> Option<&'a UiNodeRequest> {
        if node.id == id {
            return Some(node);
        }

        node.children.iter().find_map(|child| find_node(child, id))
    }

    fn aurelia_asset_preview_fixture_xml() -> String {
        #[cfg(not(miri))]
        {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

            for candidate in [
                manifest_dir
                    .join("../../../assets/ui/src/devtools/aurelia_asset_preview_stand.neui.xml"),
                manifest_dir
                    .join("../../assets/ui/src/devtools/aurelia_asset_preview_stand.neui.xml"),
            ] {
                if candidate.exists() {
                    return std::fs::read_to_string(&candidate).unwrap_or_else(|err| {
                        panic!("failed to read {}: {}", candidate.display(), err)
                    });
                }
            }
        }

        aurelia_asset_preview_embedded_fixture_xml()
    }

    fn aurelia_asset_preview_embedded_fixture_xml() -> String {
        r#"

<NeUiDictionary document_kind="surface">

  <ThemeRef ref="assets/ui/themes/northstar_editor.neui@editor_light" />

  <TextureRef ref="assets/textures/ui/loading/loading_ui.ytd@newengine_logo" />

  <TextureRef ref="assets/textures/ui/icons/builtin_icons.ytd@folder" />

  <FontRef ref="assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold" />

  <Surface name="engine.ui.aurelia_asset_preview_stand" kind="devtools" root="preview.root" theme="assets/ui/themes/northstar_editor.neui@editor_light" />

  <Layout name="preview.root" surface="engine.ui.aurelia_asset_preview_stand">

    <Panel id="preview.root">

      <Text id="preview.title" value="AureliaUI asset preview stand" font="assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold" />

      <Image id="preview.logo" texture="assets/textures/ui/loading/loading_ui.ytd@newengine_logo" />

      <Icon id="preview.folder" icon="assets/textures/ui/icons/builtin_icons.ytd@folder" />

      <Text id="preview.caption" value="Preview caption" font="assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold" />

    </Panel>

  </Layout>

  <BindingGraph id="preview.bindings">

    <StateSource id="preview" source="engine.assets" contract="asset.preview" update="frame" />

    <Bind element="preview.logo" property="texture" source_id="preview" source="texture" />

    <Bind element="preview.caption" property="text" source_id="preview" source="caption" />

  </BindingGraph>

  <Action id="ui.preview.refresh" element="preview.root" trigger="click" target="engine.ui" command="preview.refresh" />

</NeUiDictionary>

"#

        .to_owned()
    }

    #[test]
    fn compiles_aurelia_asset_preview_stand_with_font_texture_and_binding_refs() {
        let xml = aurelia_asset_preview_fixture_xml();

        let document_ref = "assets/ui/devtools/aurelia_asset_preview_stand.neui@surface";

        validate_requested_entry(&xml, "surface").unwrap();

        let dependencies = extract_dependencies(&xml);

        assert!(dependencies
            .iter()
            .any(|dep| dep == "assets/textures/ui/loading/loading_ui.ytd@newengine_logo"));

        assert!(dependencies
            .iter()
            .any(|dep| dep == "assets/textures/ui/icons/builtin_icons.ytd@folder"));

        assert!(dependencies
            .iter()
            .any(|dep| dep == "assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold"));

        let surface = parse_surface(&xml).expect("preview stand must declare a surface");

        assert_eq!(surface.name, "engine.ui.aurelia_asset_preview_stand");

        assert_eq!(
            surface.theme.as_deref(),
            Some("assets/ui/themes/northstar_editor.neui@editor_light")
        );

        let root =
            compile_surface_root(&xml, &surface, document_ref, surface.theme.as_deref()).unwrap();

        assert_eq!(root.kind, UiRuntimeNodeKind::Surface);

        assert_eq!(
            root.props.get("theme_ref").and_then(|value| value.as_str()),
            Some("assets/ui/themes/northstar_editor.neui@editor_light")
        );

        let title = find_node(&root, "preview.title").expect("title text node must compile");

        assert_eq!(title.kind, UiRuntimeNodeKind::Text);

        assert_eq!(
            title.font_token.as_deref(),
            Some("assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold")
        );

        assert_eq!(title.text, "AureliaUI asset preview stand");

        let logo = find_node(&root, "preview.logo").expect("texture image node must compile");

        assert_eq!(logo.kind, UiRuntimeNodeKind::ExternalTexture);

        assert_eq!(
            logo.icon.as_deref(),
            Some("assets/textures/ui/loading/loading_ui.ytd@newengine_logo")
        );

        let folder = find_node(&root, "preview.folder").expect("icon node must compile");

        assert_eq!(folder.kind, UiRuntimeNodeKind::ExternalTexture);

        assert_eq!(
            folder.icon.as_deref(),
            Some("assets/textures/ui/icons/builtin_icons.ytd@folder")
        );

        let caption = find_node(&root, "preview.caption").expect("caption text node must compile");

        assert_eq!(
            caption.font_token.as_deref(),
            Some("assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold")
        );

        let binding_plan = parse_binding_plan(&xml, document_ref, &surface.name);

        assert_eq!(binding_plan.state_sources.len(), 1);

        assert!(binding_plan
            .bindings
            .iter()
            .any(|binding| binding.element_id == "preview.logo" && binding.property == "texture"));

        assert!(binding_plan
            .bindings
            .iter()
            .any(|binding| binding.element_id == "preview.caption" && binding.property == "text"));

        assert!(binding_plan
            .actions
            .iter()
            .any(|action| action.action_id == "ui.preview.refresh"
                && action.target_gateway == "engine.ui"));
    }
}
