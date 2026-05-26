#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned `engine.assets.ui` semantic service.
//!
//! `.neui` is a NEF8/ListFile UI dictionary. This crate owns the UI-domain
//! meaning of that dictionary: XMLcentral validation, surface/document selection,
//! dependency extraction and runtime DTO compilation. Consumers only call the
//! `engine.assets.ui` gateway and receive a response DTO.

use abi_stable::std_types::{RResult, RString};
use flate2::read::DeflateDecoder;
use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    assets_ui_method, list_file_content_kind_label as content_kind_label,
    parse_list_file_header_v1, ASSETS_UI_BACKEND_CAPABILITY_ID, ASSETS_UI_RUNTIME_CONTRACT,
    ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS, ENGINE_ASSET_SERVICE_ID,
    ENGINE_ASSETS_UI_SERVICE_ID, LIST_FILE_CONTENT_KIND_NEUI,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use newengine_ui_api::{
    UiActionEdge, UiBindingEdge, UiBindingMode, UiBindingPlan, UiCompiledDocument, UiStateSource,
    UiUpdatePolicy,
};
use newengine_ui_navigation_api::{
    MenuActionRoute, MenuDocument, MenuFeedbackEvent, MenuFeedbackSeverity, MenuItem, MenuItemTone,
    MenuPage, MenuTransition, MenuTransitionKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;

pub const ASSETS_UI_GATEWAY_OWNER: &str = "newengine-assets-ui-runtime.engine-owned-provider";

#[derive(Clone)]
pub struct AssetsUiRuntimeState {
    client: AssetServiceClient,
    xml_cache: HashMap<String, String>,
    compile_cache: HashMap<String, AssetsUiCompileResponse>,
}

impl AssetsUiRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self { client, xml_cache: HashMap::new(), compile_cache: HashMap::new() }
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
        Self { document_ref: String::new(), ui_ref: String::new(), logical_path: String::new(), entry: String::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiCompileRequest {
    pub document_ref: String,
    pub ui_ref: String,
    pub logical_path: String,
    pub entry: String,
    pub mount_runtime: bool,
}

impl Default for AssetsUiCompileRequest {
    #[inline]
    fn default() -> Self {
        Self { document_ref: String::new(), ui_ref: String::new(), logical_path: String::new(), entry: String::new(), mount_runtime: false }
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
    pub menu_document: Option<MenuDocument>,
    pub dependencies: Vec<String>,
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
            menu_document: None,
            dependencies: Vec::new(),
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
            message: String::new(),
            warnings: Vec::new(),
        }
    }
}

pub fn assets_ui_service_info() -> AssetsUiServiceInfo {
    AssetsUiServiceInfo {
        id: ASSETS_UI_SERVICE_ID,
        gateway: ENGINE_ASSETS_UI_SERVICE_ID,
        provider: "EngineOwnedAssetsUiRuntimeProvider",
        contract: ASSETS_UI_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_UI_SERVICE_ID,
        runtime_owner: newengine_ui_api::ENGINE_UI_SERVICE_ID,
        methods: ASSETS_UI_SERVICE_METHODS,
        policy: ".neui is NEF8/ListFile with deflate XMLcentral body; consumers receive compiled DTOs by request/response and never parse bytes directly",
    }
}

fn invoke_json(state: &mut AssetsUiRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value.get("method").and_then(|v| v.as_str()).unwrap_or(assets_ui_method::COMPILE_DOCUMENT_V1);
    let request_value = value.get("request").cloned().unwrap_or_default();
    match method {
        assets_ui_method::COMPILE_DOCUMENT_V1 => {
            let request = serde_json::from_value::<AssetsUiCompileRequest>(request_value).unwrap_or_default();
            match compile_document(state, request) {
                Ok(response) => ok_json(response),
                Err(e) => ok_json(error_response_from_message(e)),
            }
        }
        assets_ui_method::DOCUMENT_V1 | assets_ui_method::DUMP_XMLCENTRAL_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();
            match load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => ok_json(serde_json::json!({
                    "ok": true,
                    "schema": "newengine.assets.ui.document.response.v1",
                    "document_ref": resolved.document_ref,
                    "logical_path": resolved.logical_path,
                    "vfs_path": resolved.vfs_path,
                    "entry": resolved.entry,
                    "xmlcentral": xml,
                })),
                Err(e) => ok_json(error_response_from_message(e)),
            }
        }
        assets_ui_method::VALIDATE_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();
            match load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => ok_json(AssetsUiDiagnosticResponse {
                    ok: true,
                    document_ref: resolved.document_ref,
                    logical_path: resolved.logical_path,
                    entry: resolved.entry,
                    message: format!("valid .neui XMLcentral bytes={} root={}", xml.len(), root_name(&xml).unwrap_or("unknown")),
                    ..Default::default()
                }),
                Err(e) => ok_json(error_response_from_message(e)),
            }
        }
        assets_ui_method::DEPENDENCIES_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();
            match load_xmlcentral(state, request) {
                Ok((xml, _, resolved)) => ok_json(serde_json::json!({
                    "ok": true,
                    "schema": "newengine.assets.ui.dependencies.response.v1",
                    "document_ref": resolved.document_ref,
                    "logical_path": resolved.logical_path,
                    "entry": resolved.entry,
                    "dependencies": extract_dependencies(&xml),
                })),
                Err(e) => ok_json(error_response_from_message(e)),
            }
        }
        assets_ui_method::MANIFEST_V1 | assets_ui_method::ENTRY_V1 | assets_ui_method::REGISTRY_V1 | assets_ui_method::BINDING_PLAN_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request_value).unwrap_or_default();
            match compile_document(state, compile_request_from_ref(request)) {
                Ok(response) => ok_json(response),
                Err(e) => ok_json(error_response_from_message(e)),
            }
        }
        other => RResult::RErr(RString::from(format!("engine.assets.ui: unknown invoke_json method '{other}'"))),
    }
}

pub fn assets_ui_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        ASSETS_UI_SERVICE_ID,
        ASSETS_UI_GATEWAY_OWNER,
        ASSETS_UI_BACKEND_CAPABILITY_ID,
        ASSETS_UI_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_UI_SERVICE_ID)
    .protocol(ASSETS_UI_RUNTIME_CONTRACT)
    .features(["neui-nef8-xmlcentral", "compile-document-v1", "menu-document-dto", "dependency-extraction"])
    .notes("Engine UI asset semantic service. Consumers call engine.assets.ui and receive runtime DTOs; engine.ui owns only live mount/state/input/draw runtime.");

    JsonServiceRouter::with_state(ASSETS_UI_SERVICE_ID, AssetsUiRuntimeState::new(client))
        .describe_json(&description)
        .info(assets_ui_service_info)
        .post_json_result::<AssetsUiCompileRequest, AssetsUiCompileResponse, _>(assets_ui_method::COMPILE_DOCUMENT_V1, compile_document)
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(assets_ui_method::DOCUMENT_V1, |state, request| {
            let (xml, _, resolved) = load_xmlcentral(state, request)?;
            Ok(serde_json::json!({
                "ok": true,
                "schema": "newengine.assets.ui.document.response.v1",
                "document_ref": resolved.document_ref,
                "logical_path": resolved.logical_path,
                "vfs_path": resolved.vfs_path,
                "entry": resolved.entry,
                "xmlcentral": xml,
            }))
        })
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(assets_ui_method::DUMP_XMLCENTRAL_V1, |state, request| {
            let (xml, _, resolved) = load_xmlcentral(state, request)?;
            Ok(serde_json::json!({
                "ok": true,
                "schema": "newengine.assets.ui.xmlcentral_dump.v1",
                "document_ref": resolved.document_ref,
                "logical_path": resolved.logical_path,
                "vfs_path": resolved.vfs_path,
                "entry": resolved.entry,
                "xmlcentral": xml,
            }))
        })
        .post_json_result::<AssetsUiRefRequest, AssetsUiDiagnosticResponse, _>(assets_ui_method::VALIDATE_V1, |state, request| {
            let (xml, _, resolved) = load_xmlcentral(state, request)?;
            Ok(AssetsUiDiagnosticResponse {
                ok: true,
                document_ref: resolved.document_ref,
                logical_path: resolved.logical_path,
                entry: resolved.entry,
                message: format!("valid .neui XMLcentral bytes={} root={}", xml.len(), root_name(&xml).unwrap_or("unknown")),
                ..Default::default()
            })
        })
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(assets_ui_method::DEPENDENCIES_V1, |state, request| {
            let (xml, _, resolved) = load_xmlcentral(state, request)?;
            Ok(serde_json::json!({
                "ok": true,
                "schema": "newengine.assets.ui.dependencies.response.v1",
                "document_ref": resolved.document_ref,
                "logical_path": resolved.logical_path,
                "entry": resolved.entry,
                "dependencies": extract_dependencies(&xml),
            }))
        })
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(assets_ui_method::MANIFEST_V1, |state, request| compile_document(state, compile_request_from_ref(request)))
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(assets_ui_method::ENTRY_V1, |state, request| compile_document(state, compile_request_from_ref(request)))
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(assets_ui_method::REGISTRY_V1, |state, request| compile_document(state, compile_request_from_ref(request)))
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(assets_ui_method::BINDING_PLAN_V1, |state, request| compile_document(state, compile_request_from_ref(request)))
        .blob(assets_ui_method::INVOKE_JSON, invoke_json)
        .blob(assets_ui_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_assets_ui_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_ASSETS_UI_SERVICE_ID,
        service_kind: EngineServiceKind::AssetUi,
        provider_service: ASSETS_UI_SERVICE_ID,
        capability: ASSETS_UI_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ASSETS_UI_GATEWAY_OWNER,
        service: assets_ui_gateway_service(client),
    })
}

fn compile_request_from_ref(request: AssetsUiRefRequest) -> AssetsUiCompileRequest {
    AssetsUiCompileRequest {
        document_ref: first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]),
        ui_ref: String::new(),
        logical_path: request.logical_path,
        entry: request.entry,
        mount_runtime: false,
    }
}

fn error_response_from_message(message: String) -> AssetsUiDiagnosticResponse {
    AssetsUiDiagnosticResponse { message, ..Default::default() }
}

#[derive(Clone, Debug)]
struct ResolvedUiRef {
    document_ref: String,
    logical_path: String,
    vfs_path: String,
    entry: String,
}

fn compile_document(state: &mut AssetsUiRuntimeState, request: AssetsUiCompileRequest) -> Result<AssetsUiCompileResponse, String> {
    let cache_key = canonical_request_ref(&request);
    if let Some(cached) = state.compile_cache.get(&cache_key) {
        return Ok(cached.clone());
    }

    let ref_request = AssetsUiRefRequest {
        document_ref: request.document_ref.clone(),
        ui_ref: request.ui_ref.clone(),
        logical_path: request.logical_path.clone(),
        entry: request.entry.clone(),
    };
    let (xml, warnings, resolved) = load_xmlcentral(state, ref_request)?;
    validate_requested_entry(&xml, &resolved.entry)?;

    let surface = parse_surface(&xml).ok_or_else(|| format!(".neui '{}' has no <Surface> entry", resolved.document_ref))?;
    let dependencies = extract_dependencies(&xml);
    let binding_plan = parse_binding_plan(&xml, &resolved.document_ref, &surface.name);
    let compiled_document = UiCompiledDocument {
        version: 1,
        document_ref: resolved.document_ref.clone(),
        surface_id: surface.name.clone(),
        root_id: surface.root.clone(),
        theme_ref: surface.theme.clone(),
        dependencies: dependencies.clone(),
        binding_plan,
    };
    let menu_document = parse_menu_document(&xml)?;

    let response = AssetsUiCompileResponse {
        ok: true,
        document_ref: resolved.document_ref.clone(),
        logical_path: resolved.logical_path.clone(),
        vfs_path: resolved.vfs_path.clone(),
        entry: resolved.entry.clone(),
        surface_id: surface.name,
        xmlcentral: xml,
        compiled_document,
        menu_document,
        dependencies,
        warnings,
        ..Default::default()
    };
    state.compile_cache.insert(cache_key, response.clone());
    Ok(response)
}

fn load_xmlcentral(state: &mut AssetsUiRuntimeState, request: AssetsUiRefRequest) -> Result<(String, Vec<String>, ResolvedUiRef), String> {
    let resolved = resolve_ui_ref(request)?;
    if let Some(xml) = state.xml_cache.get(&resolved.vfs_path) {
        return Ok((xml.clone(), Vec::new(), resolved));
    }

    let mut warnings = Vec::new();
    let mut last_err = None;
    for candidate in vfs_candidates(&resolved.logical_path) {
        match state.client.raw_bytes_v1(&candidate) {
            Ok(bytes) => {
                let xml = decode_neui_xmlcentral(&candidate, &bytes)?;
                state.xml_cache.insert(candidate.clone(), xml.clone());
                let actual = ResolvedUiRef { vfs_path: candidate, ..resolved };
                return Ok((xml, warnings, actual));
            }
            Err(e) => {
                last_err = Some(format!("{}: {}", candidate, e));
            }
        }
    }
    warnings.push("VFS lookup tried both literal and assets/-stripped paths".to_owned());
    Err(format!("engine.assets.ui could not read .neui bytes for '{}': {}", resolved.document_ref, last_err.unwrap_or_else(|| "no candidate path".to_owned())))
}

fn decode_neui_xmlcentral(logical_path: &str, bytes: &[u8]) -> Result<String, String> {
    let header = parse_list_file_header_v1(bytes)?;
    if header.content_kind != LIST_FILE_CONTENT_KIND_NEUI {
        return Err(format!(
            "{} is NEF8 content_kind='{}' ({}) not ui_dictionary ({})",
            logical_path,
            content_kind_label(header.content_kind),
            header.content_kind,
            LIST_FILE_CONTENT_KIND_NEUI
        ));
    }
    let start = usize::try_from(header.body_offset).map_err(|_| "NEF8 body_offset does not fit usize".to_owned())?;
    let len = usize::try_from(header.body_len).map_err(|_| "NEF8 body_len does not fit usize".to_owned())?;
    let end = start.checked_add(len).ok_or_else(|| "NEF8 body range overflow".to_owned())?;
    let compressed = bytes.get(start..end).ok_or_else(|| format!("NEF8 body range outside file: offset={} len={} file={}", start, len, bytes.len()))?;

    let mut decoder = DeflateDecoder::new(compressed);
    let mut body = Vec::with_capacity(header.body_uncompressed_len as usize);
    decoder.read_to_end(&mut body).map_err(|e| format!("NEF8 deflate body decode failed: {e}"))?;
    if body.len() != header.body_uncompressed_len as usize {
        return Err(format!("NEF8 inflated body length mismatch: got={} expected={}", body.len(), header.body_uncompressed_len));
    }
    let hash = blake3::hash(&body);
    if header.body_raw_hash != *hash.as_bytes() {
        return Err("NEF8 inflated body BLAKE3 hash mismatch".to_owned());
    }
    let xml = String::from_utf8(body).map_err(|e| format!(".neui XMLcentral body is not UTF-8: {e}"))?;
    let Some(root) = root_name(&xml) else {
        return Err(".neui XMLcentral body has no root element".to_owned());
    };
    if !matches!(root, "NeUiDictionary" | "NeUiRegistry" | "NeUiThemeLibrary" | "NeUiComponentLibrary" | "NeUiBindingLibrary") {
        return Err(format!("unsupported .neui XMLcentral root '{root}'"));
    }
    Ok(xml)
}

fn resolve_ui_ref(request: AssetsUiRefRequest) -> Result<ResolvedUiRef, String> {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    let (path, entry) = if !combined.trim().is_empty() {
        split_ref(&combined)
    } else {
        let path = normalize_logical_path(&request.logical_path);
        let entry = normalize_entry(&request.entry);
        (path, entry)
    };
    if path.is_empty() {
        return Err("engine.assets.ui request requires document_ref='path.neui@entry' or logical_path".to_owned());
    }
    if !path.to_ascii_lowercase().ends_with(&format!(".{}", newengine_asset_format_nef8::neui::EXTENSION)) {
        return Err(format!("engine.assets.ui accepts only .{} dictionaries, got '{path}'", newengine_asset_format_nef8::neui::EXTENSION));
    }
    let entry = if entry.is_empty() { "surface".to_owned() } else { entry };
    Ok(ResolvedUiRef {
        document_ref: format!("{}@{}", path, entry),
        logical_path: path.clone(),
        vfs_path: path,
        entry,
    })
}

fn canonical_request_ref(request: &AssetsUiCompileRequest) -> String {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    if !combined.is_empty() {
        let (path, entry) = split_ref(&combined);
        return format!("{}@{}", path, if entry.is_empty() { "surface" } else { &entry });
    }
    format!("{}@{}", normalize_logical_path(&request.logical_path), normalize_entry(&request.entry))
}

fn split_ref(value: &str) -> (String, String) {
    let normalized = normalize_logical_path(value);
    if let Some((path, entry)) = normalized.split_once('@') {
        (normalize_logical_path(path), normalize_entry(entry))
    } else {
        (normalized, String::new())
    }
}

fn normalize_logical_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

fn normalize_entry(value: &str) -> String {
    value.trim().trim_start_matches('@').trim().to_owned()
}

fn vfs_candidates(path: &str) -> Vec<String> {
    let normalized = normalize_logical_path(path);
    let mut out = vec![normalized.clone()];
    if let Some(stripped) = normalized.strip_prefix("assets/") {
        out.push(stripped.to_owned());
    } else {
        out.push(format!("assets/{normalized}"));
    }
    out.sort();
    out.dedup();
    out
}

fn validate_requested_entry(xml: &str, entry: &str) -> Result<(), String> {
    if entry.trim().is_empty() || entry == "surface" {
        return Ok(());
    }
    let entries_section = section(xml, "Entries").unwrap_or_default();
    let entries = elements(&entries_section, "Entry");
    if entries.iter().any(|element| attr_value(&element.open, "name").as_deref() == Some(entry)) {
        Ok(())
    } else {
        Err(format!(".neui entry '@{}' is not declared in <Entries>", entry))
    }
}

#[derive(Clone, Debug)]
struct SurfaceInfo {
    name: String,
    root: String,
    theme: Option<String>,
}

fn parse_surface(xml: &str) -> Option<SurfaceInfo> {
    let element = first_element(xml, "Surface")?;
    let name = attr_value(&element.open, "name").unwrap_or_else(|| "engine.unknown".to_owned());
    let root = attr_value(&element.open, "root").unwrap_or_else(|| "layout.main".to_owned());
    let theme = attr_value(&element.open, "theme").filter(|value| !value.trim().is_empty());
    Some(SurfaceInfo { name, root, theme })
}

fn extract_dependencies(xml: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for tag in ["ThemeRef", "ComponentRef", "TextureRef", "FontRef", "SoundRef", "BindingGraphRef", "DocumentRef"] {
        for element in elements(xml, tag) {
            if let Some(reference) = attr_value(&element.open, "ref") {
                if !reference.trim().is_empty() {
                    deps.push(reference);
                }
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn parse_binding_plan(xml: &str, document_ref: &str, surface_id: &str) -> UiBindingPlan {
    let mut plan = UiBindingPlan { document_ref: document_ref.to_owned(), surface_id: surface_id.to_owned(), ..Default::default() };
    if let Some(graph) = first_element(xml, "BindingGraph") {
        for source in elements(&graph.inner, "StateSource") {
            plan.state_sources.push(UiStateSource {
                id: attr_value(&source.open, "id").unwrap_or_default(),
                source: attr_value(&source.open, "source").unwrap_or_default(),
                contract: attr_value(&source.open, "contract").unwrap_or_default(),
                update_policy: update_policy_from_attr(attr_value(&source.open, "update").as_deref()),
            });
        }
        for bind in elements(&graph.inner, "Bind") {
            plan.bindings.push(UiBindingEdge {
                element_id: attr_value(&bind.open, "element").unwrap_or_default(),
                property: attr_value(&bind.open, "property").unwrap_or_default(),
                source_id: attr_value(&bind.open, "source_id").unwrap_or_default(),
                path: attr_value(&bind.open, "source").unwrap_or_default(),
                mode: UiBindingMode::OneWay,
                fallback: attr_value(&bind.open, "fallback"),
                transform: attr_value(&bind.open, "transform"),
            });
        }
    }
    for action in elements(xml, "Action") {
        if let Some(action_id) = attr_value(&action.open, "id") {
            plan.actions.push(UiActionEdge {
                element_id: attr_value(&action.open, "element").unwrap_or_default(),
                trigger: attr_value(&action.open, "trigger").unwrap_or_else(|| "click".to_owned()),
                action_id,
                target_gateway: attr_value(&action.open, "target").unwrap_or_default(),
                command: attr_value(&action.open, "command").or_else(|| attr_value(&action.open, "event")).unwrap_or_default(),
                payload_schema: None,
            });
        }
    }
    plan
}

fn update_policy_from_attr(value: Option<&str>) -> UiUpdatePolicy {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "frame" => UiUpdatePolicy::Frame,
        "event" => UiUpdatePolicy::Event,
        "dirty" => UiUpdatePolicy::Dirty,
        "manual" => UiUpdatePolicy::Manual,
        _ => UiUpdatePolicy::OnChange,
    }
}

fn parse_menu_document(xml: &str) -> Result<Option<MenuDocument>, String> {
    let Some(menu) = first_element(xml, "MenuDocument") else {
        return Ok(None);
    };
    let mut doc = MenuDocument {
        id: attr_value(&menu.open, "id").unwrap_or_else(|| "engine.pause_menu".to_owned()),
        version: attr_value(&menu.open, "version").and_then(|v| v.parse().ok()).unwrap_or(1),
        surface_id: attr_value(&menu.open, "surface_id").or_else(|| attr_value(&menu.open, "surface")).unwrap_or_else(|| "engine.pause_menu".to_owned()),
        root_page: attr_value(&menu.open, "root_page").unwrap_or_else(|| "root".to_owned()),
        title: attr_value(&menu.open, "title").unwrap_or_default(),
        subtitle: attr_value(&menu.open, "subtitle").unwrap_or_default(),
        footer_lines: Vec::new(),
        pages: Vec::new(),
    };

    if let Some(footer) = first_element(&menu.inner, "Footer") {
        for line in elements(&footer.inner, "Line") {
            if let Some(value) = attr_value(&line.open, "value") {
                if !value.trim().is_empty() {
                    doc.footer_lines.push(value);
                }
            }
        }
    }

    for page_element in elements(&menu.inner, "Page") {
        let mut page = MenuPage {
            id: attr_value(&page_element.open, "id").unwrap_or_default(),
            title: attr_value(&page_element.open, "title").unwrap_or_default(),
            subtitle: attr_value(&page_element.open, "subtitle").unwrap_or_default(),
            parent_page: attr_value(&page_element.open, "parent_page"),
            footer_lines: Vec::new(),
            items: Vec::new(),
            back_route: first_route_element(&page_element.inner, "Back"),
        };
        if let Some(footer) = first_element(&page_element.inner, "Footer") {
            for line in elements(&footer.inner, "Line") {
                if let Some(value) = attr_value(&line.open, "value") {
                    page.footer_lines.push(value);
                }
            }
        }
        for item_element in elements(&page_element.inner, "Item") {
            let item = MenuItem {
                id: attr_value(&item_element.open, "id").unwrap_or_default(),
                label: attr_value(&item_element.open, "label").unwrap_or_default(),
                value: attr_value(&item_element.open, "value"),
                detail: attr_value(&item_element.open, "detail"),
                emphasized: bool_attr(&item_element.open, "emphasized"),
                tone: tone_from_attr(attr_value(&item_element.open, "tone").as_deref()),
                dynamic_value: attr_value(&item_element.open, "dynamic_value"),
                action: first_route_element(&item_element.inner, "Action"),
                nav_left: first_route_element(&item_element.inner, "NavLeft"),
                nav_right: first_route_element(&item_element.inner, "NavRight"),
            };
            page.items.push(item);
        }
        doc.pages.push(page);
    }
    doc = doc.canonicalized();
    doc.validate()?;
    Ok(Some(doc))
}

fn first_route_element(xml: &str, name: &str) -> Option<MenuActionRoute> {
    let element = first_element(xml, name)?;
    Some(route_from_element(&element))
}

fn route_from_element(element: &XmlElement) -> MenuActionRoute {
    let mut payload = BTreeMap::new();
    if let Some(page) = attr_value(&element.open, "page") {
        payload.insert("page".to_owned(), serde_json::Value::String(page));
    }
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(value));
        }
    }
    MenuActionRoute {
        id: attr_value(&element.open, "id").unwrap_or_default(),
        source: attr_value(&element.open, "source").unwrap_or_default(),
        target: attr_value(&element.open, "target").unwrap_or_else(|| "MenuRuntime".to_owned()),
        event: attr_value(&element.open, "event").unwrap_or_else(|| event_from_route_tag(&element.name).to_owned()),
        payload,
        transition: transition_from_attrs(&element.open),
        feedback: first_element(&element.inner, "Feedback").map(|feedback| MenuFeedbackEvent {
            title: attr_value(&feedback.open, "title").unwrap_or_default(),
            detail: attr_value(&feedback.open, "detail").unwrap_or_default(),
            severity: feedback_severity_from_attr(attr_value(&feedback.open, "severity").as_deref()),
            ttl_sec: attr_value(&feedback.open, "ttl_sec").and_then(|v| v.parse().ok()).unwrap_or(2.25),
        }),
        audio: attr_value(&element.open, "audio"),
    }
}

fn event_from_route_tag(name: &str) -> &'static str {
    match name {
        "Back" => "menu.back",
        "NavLeft" => "menu.nav_left",
        "NavRight" => "menu.nav_right",
        _ => "menu.activate",
    }
}

fn transition_from_attrs(open: &str) -> Option<MenuTransition> {
    match attr_value(open, "transition").unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "close" => Some(MenuTransition::close()),
        "open_page" => attr_value(open, "page").map(MenuTransition::open_page),
        "back" => Some(MenuTransition { kind: MenuTransitionKind::Back, page: None, reset_selection: true }),
        "none" | "" => None,
        _ => None,
    }
}

fn tone_from_attr(value: Option<&str>) -> MenuItemTone {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "accent" => MenuItemTone::Accent,
        "danger" => MenuItemTone::Danger,
        "disabled" => MenuItemTone::Disabled,
        _ => MenuItemTone::Normal,
    }
}

fn feedback_severity_from_attr(value: Option<&str>) -> MenuFeedbackSeverity {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "success" => MenuFeedbackSeverity::Success,
        "warning" => MenuFeedbackSeverity::Warning,
        "danger" | "error" => MenuFeedbackSeverity::Danger,
        _ => MenuFeedbackSeverity::Info,
    }
}

fn bool_attr(open: &str, key: &str) -> bool {
    matches!(attr_value(open, key).unwrap_or_default().trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

#[derive(Clone, Debug)]
struct XmlElement {
    name: String,
    open: String,
    inner: String,
}

fn root_name(xml: &str) -> Option<&str> {
    let mut rest = xml.trim_start();
    if rest.starts_with("<?") {
        let end = rest.find("?>")?;
        rest = rest.get(end + 2..)?.trim_start();
    }
    let open = rest.strip_prefix('<')?;
    let name_end = open.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    open.get(..name_end)
}

fn section(xml: &str, name: &str) -> Option<String> {
    first_element(xml, name).map(|element| element.inner)
}

fn first_element(xml: &str, name: &str) -> Option<XmlElement> {
    elements(xml, name).into_iter().next()
}

fn elements(xml: &str, name: &str) -> Vec<XmlElement> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(start_rel) = find_open_tag(&xml[offset..], name) {
        let start = offset + start_rel;
        let Some(open_end_rel) = xml[start..].find('>') else { break; };
        let open_end = start + open_end_rel;
        let open = &xml[start..=open_end];
        let self_closing = open.trim_end().ends_with("/>");
        if self_closing {
            out.push(XmlElement { name: name.to_owned(), open: open.to_owned(), inner: String::new() });
            offset = open_end + 1;
            continue;
        }
        let close_token = format!("</{}>", name);
        let Some(close_rel) = xml[open_end + 1..].find(&close_token) else { break; };
        let inner_start = open_end + 1;
        let close_start = inner_start + close_rel;
        let close_end = close_start + close_token.len();
        out.push(XmlElement {
            name: name.to_owned(),
            open: open.to_owned(),
            inner: xml[inner_start..close_start].to_owned(),
        });
        offset = close_end;
    }
    out
}

fn find_open_tag(haystack: &str, name: &str) -> Option<usize> {
    let needle = format!("<{}", name);
    let mut search = 0usize;
    while let Some(pos_rel) = haystack[search..].find(&needle) {
        let pos = search + pos_rel;
        let next = haystack.as_bytes().get(pos + needle.len()).copied();
        if matches!(next, Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')) {
            return Some(pos);
        }
        search = pos + needle.len();
    }
    None
}

fn attr_value(open: &str, key: &str) -> Option<String> {
    parse_attrs(open).remove(key).map(|value| xml_unescape(&value))
}

fn parse_attrs(open: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let bytes = open.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'\n' && bytes[i] != b'\r' && bytes[i] != b'>' && bytes[i] != b'/' {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' { break; }
        let key_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-' || bytes[i] == b'.' || bytes[i] == b':') { i += 1; }
        let key = open[key_start..i].trim();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() || bytes[i] != b'=' { continue; }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() || (bytes[i] != b'\"' && bytes[i] != b'\'') { continue; }
        let quote = bytes[i];
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote { i += 1; }
        if i <= bytes.len() && !key.is_empty() {
            attrs.insert(key.to_owned(), open[value_start..i].to_owned());
        }
        i = i.saturating_add(1);
    }
    attrs
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_menu_document_from_xmlcentral() {
        let xml = r#"<NeUiDictionary><MenuDocument id="engine.pause_menu" surface_id="engine.pause_menu" root_page="root" title="PAUSE"><Page id="root"><Item id="resume" label="Resume"><Action id="resume" source="s" target="MenuRuntime" event="menu.close" transition="close" /></Item></Page></MenuDocument></NeUiDictionary>"#;
        let doc = parse_menu_document(xml).unwrap().unwrap();
        assert_eq!(doc.id, "engine.pause_menu");
        assert_eq!(doc.pages[0].items[0].label, "Resume");
    }
}
