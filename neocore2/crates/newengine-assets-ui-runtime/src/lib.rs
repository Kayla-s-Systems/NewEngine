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
    ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS, ENGINE_ASSET_SERVICE_ID,
    ENGINE_ASSETS_UI_SERVICE_ID, LIST_FILE_CONTENT_KIND_NEUI,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_ui_api::{
    UiActionEdge, UiBindingEdge, UiBindingMode, UiBindingPlan, UiCompiledDocument,
    UiComponentLibraryRef, UiComponentTemplate, UiDocumentSource, UiDocumentSourceKind,
    UiNodeBindingRequest, UiNodeEventRoute, UiNodeEventTrigger, UiNodeRequest,
    UiNodeTone, UiRuntimeNodeKind, UiSourceSpan, UiStateSource, UiThemeLibraryRef,
    UiUpdatePolicy, UI_COMPONENT_ACTION, UI_COMPONENT_BUTTON, UI_COMPONENT_CHECKBOX,
    UI_COMPONENT_EXTERNAL_TEXTURE, UI_COMPONENT_GRID, UI_COMPONENT_INPUT, UI_COMPONENT_LIST,
    UI_COMPONENT_ROW, UI_COMPONENT_SCROLL_BAR, UI_COMPONENT_SELECT,
    UI_COMPONENT_SEPARATOR, UI_COMPONENT_SLIDER, UI_COMPONENT_SPACER, UI_COMPONENT_STACK,
    UI_COMPONENT_SURFACE, UI_COMPONENT_TEXT, UI_COMPONENT_TOGGLE, UI_COMPONENT_TREE,
    UI_COMPONENT_VIEWPORT,
};
use newengine_ui_navigation_api::{
    UiNodeActionRoute, UiNodeNavigationDocument, UiNodeFeedbackEvent, UiNodeFeedbackSeverity, UiNodeNavigationItem, UiNodeNavigationTone,
    UiNodeNavigationPage, UiNodeTransition, UiNodeTransitionKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;

pub const ASSETS_UI_GATEWAY_OWNER: &str = "newengine-assets-ui-runtime.engine-runtime-provider";

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
    let method = value.get("method").and_then(|v| v.as_str()).unwrap_or(assets_ui_method::COMPILE_DOCUMENT_V1);
    let request_value = value.get("request").cloned().unwrap_or_default();
    match method {
        assets_ui_method::COMPILE_DOCUMENT_V1 => {
            let request = serde_json::from_value::<AssetsUiCompileRequest>(request_value).unwrap_or_default();
            match compile_document(state, request.clone()) {
                Ok(response) => ok_json(response),
                Err(e) => ok_json(error_response_from_compile_error(e, &request)),
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
                Ok((xml, _, resolved)) => {
                    let source_span = source_span_for_offset(&xml, 0, &resolved.document_ref);
                    ok_json(AssetsUiDiagnosticResponse {
                        ok: true,
                        document_ref: resolved.document_ref,
                        logical_path: resolved.logical_path,
                        entry_id: resolved.entry.clone(),
                        entry: resolved.entry,
                        source_span,
                        message: format!("valid binary .neui decoded to XMLcentral bytes={} root={}", xml.len(), root_name(&xml).unwrap_or("unknown")),
                        ..Default::default()
                    })
                },
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
                message: format!("valid binary .neui decoded to XMLcentral bytes={} root={}", xml.len(), root_name(&xml).unwrap_or("unknown")),
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

fn compile_request_from_ref(request: AssetsUiRefRequest) -> AssetsUiCompileRequest {
    AssetsUiCompileRequest {
        document_ref: first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]),
        ui_ref: String::new(),
        logical_path: request.logical_path,
        entry: request.entry,
        mount_runtime: false,
        ..Default::default()
    }
}

fn error_response_from_message(message: String) -> AssetsUiDiagnosticResponse {
    AssetsUiDiagnosticResponse { message, ..Default::default() }
}

fn error_response_from_compile_error(message: String, request: &AssetsUiCompileRequest) -> AssetsUiDiagnosticResponse {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    let (path, entry) = if !combined.trim().is_empty() {
        split_ref(&combined)
    } else {
        (normalize_logical_path(&request.logical_path), normalize_entry(&request.entry))
    };
    let entry = if entry.trim().is_empty() { "surface".to_owned() } else { entry };
    AssetsUiDiagnosticResponse {
        document_ref: if path.trim().is_empty() { String::new() } else { format!("{}@{}", path, entry) },
        logical_path: path.clone(),
        entry: entry.clone(),
        entry_id: entry,
        source_span: UiSourceSpan { source_ref: path, line: 0, column: 0 },
        message,
        ..Default::default()
    }
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
    let (xml, mut warnings, resolved) = load_xmlcentral(state, ref_request)?;
    validate_requested_entry(&xml, &resolved.entry).map_err(|err| {
        let span = source_span_for_named_element(&xml, "Entries", &resolved.document_ref);
        format!(
            "{} entry='@{}' {}: {}",
            resolved.document_ref,
            resolved.entry,
            span.display(&resolved.document_ref),
            err
        )
    })?;

    let surface = parse_surface(&xml).ok_or_else(|| {
        let span = source_span_for_offset(&xml, 0, &resolved.document_ref);
        format!("{} entry='@{}' {}: .neui document has no <Surface> entry", resolved.document_ref, resolved.entry, span.display(&resolved.document_ref))
    })?;
    let mut dependencies = extract_dependencies(&xml);
    let inferred_style_ref = request
        .style_ref
        .clone()
        .or_else(|| surface.theme.clone())
        .or_else(|| first_dependency_with_suffix(&dependencies, ".neuis"))
        .or_else(|| first_dependency_with_suffix(&dependencies, ".neui@theme"));
    if let Some(style_ref) = &inferred_style_ref {
        if !dependencies.iter().any(|dep| dep == style_ref) {
            dependencies.push(style_ref.clone());
            dependencies.sort();
            dependencies.dedup();
        }
    }
    let style_dependencies = inferred_style_ref.iter().cloned().collect::<Vec<_>>();
    let binding_plan = parse_binding_plan(&xml, &resolved.document_ref, &surface.name);
    let component_libraries = parse_component_libraries(&xml);
    let theme_libraries = parse_theme_libraries(&xml, surface.theme.as_deref());
    let local_component_templates = parse_component_templates(&xml, &resolved.document_ref);
    let imported_component_templates = resolve_imported_component_templates(state, &component_libraries, &mut warnings);
    let component_templates = merge_component_templates(imported_component_templates, local_component_templates);
    let theme_tokens = resolve_theme_token_bundle(state, &theme_libraries, inferred_style_ref.as_deref(), &mut warnings);
    let mut root = compile_surface_root(&xml, &surface, &resolved.document_ref, inferred_style_ref.as_deref())?;
    if let Some(tokens) = theme_tokens.as_ref() {
        root.props.insert("theme_tokens".to_owned(), serde_json::to_value(tokens).unwrap_or(serde_json::Value::Null));
        root.style_tags.push(format!("density:{}", sanitize_tag(&tokens.density)));
        root.style_tags.sort();
        root.style_tags.dedup();
    }
    warnings.push(format!(
        ".neui live root compiled source='{}' entry='@{}' surface='{}' root_node='{}' children={} component_libraries={} theme_libraries={} component_templates={} theme_tokens={}",
        resolved.document_ref,
        resolved.entry,
        surface.name,
        root.id,
        root.children.len(),
        component_libraries.len(),
        theme_libraries.len(),
        component_templates.len(),
        theme_tokens.as_ref().map(|tokens| tokens.theme_id.as_str()).unwrap_or("<none>")
    ));
    let source = UiDocumentSource {
        kind: request.source_kind,
        document_ref: resolved.document_ref.clone(),
        style_ref: inferred_style_ref.clone(),
        stream_id: request.stream_id.clone(),
        generator_id: request.generator_id.clone(),
    };
    let compiled_document = UiCompiledDocument {
        version: 1,
        source: source.clone(),
        document_ref: resolved.document_ref.clone(),
        surface_id: surface.name.clone(),
        root_id: surface.root.clone(),
        theme_ref: surface.theme.clone(),
        style_ref: inferred_style_ref.clone(),
        dependencies: dependencies.clone(),
        style_dependencies: style_dependencies.clone(),
        component_libraries,
        theme_libraries,
        component_templates,
        root: Some(root),
        binding_plan,
        validation: Default::default(),
        dependency_report: Default::default(),
    };
    let navigation_document = match parse_navigation_document(&xml)? {
        Some(document) => Some(document),
        None => derive_navigation_document_from_surface_layout(&xml, &surface)?,
    };

    let response = AssetsUiCompileResponse {
        ok: true,
        document_ref: resolved.document_ref.clone(),
        logical_path: resolved.logical_path.clone(),
        vfs_path: resolved.vfs_path.clone(),
        entry: resolved.entry.clone(),
        surface_id: surface.name,
        xmlcentral: xml,
        compiled_document,
        navigation_document,
        source_kind: request.source_kind,
        style_ref: inferred_style_ref,
        dependencies,
        style_dependencies,
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
    let mut out = Vec::with_capacity(2);
    if let Some(stripped) = normalized.strip_prefix("assets/") {
        out.push(stripped.to_owned());
        out.push(normalized);
    } else {
        out.push(normalized.clone());
        out.push(format!("assets/{normalized}"));
    }
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
    kind: String,
    root: String,
    theme: Option<String>,
    modal: bool,
    z_order: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ThemeTokenBundle {
    theme_ref: String,
    theme_id: String,
    density: String,
    tokens: BTreeMap<String, String>,
    colors: BTreeMap<String, [u8; 4]>,
    metrics: BTreeMap<String, f32>,
    font_tokens: BTreeMap<String, String>,
}

fn parse_surface(xml: &str) -> Option<SurfaceInfo> {
    let element = first_element(xml, "Surface")?;
    let name = attr_value(&element.open, "name").unwrap_or_else(|| "engine.unknown".to_owned());
    let kind = attr_value(&element.open, "kind").unwrap_or_else(|| "surface".to_owned());
    let root = attr_value(&element.open, "root").unwrap_or_else(|| "layout.main".to_owned());
    let theme = attr_value(&element.open, "theme").filter(|value| !value.trim().is_empty());
    let modal = bool_attr(&element.open, "modal");
    let z_order = attr_value(&element.open, "z_order")
        .or_else(|| attr_value(&element.open, "z"))
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);
    Some(SurfaceInfo { name, kind, root, theme, modal, z_order })
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

fn first_dependency_with_suffix(dependencies: &[String], suffix: &str) -> Option<String> {
    dependencies
        .iter()
        .find(|dep| dep.to_ascii_lowercase().contains(&suffix.to_ascii_lowercase()))
        .cloned()
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

fn parse_component_libraries(xml: &str) -> Vec<UiComponentLibraryRef> {
    let mut libraries = BTreeMap::<String, Vec<String>>::new();
    for element in elements(xml, "ComponentRef") {
        let Some(reference) = attr_value(&element.open, "ref").filter(|it| !it.trim().is_empty()) else {
            continue;
        };
        let (library_ref, entry) = split_ref(&reference);
        if entry.trim().is_empty() {
            libraries.entry(library_ref).or_default();
        } else {
            libraries.entry(library_ref).or_default().push(entry);
        }
    }
    libraries
        .into_iter()
        .map(|(library_ref, mut entries)| {
            entries.sort();
            entries.dedup();
            UiComponentLibraryRef { library_ref, entries }
        })
        .collect()
}

fn parse_theme_libraries(xml: &str, surface_theme: Option<&str>) -> Vec<UiThemeLibraryRef> {
    let mut themes = BTreeMap::<String, Vec<String>>::new();
    if let Some(theme) = surface_theme.filter(|it| !it.trim().is_empty()) {
        let (theme_ref, entry) = split_ref(theme);
        if entry.trim().is_empty() {
            themes.entry(theme_ref).or_default();
        } else {
            themes.entry(theme_ref).or_default().push(entry);
        }
    }
    for element in elements(xml, "ThemeRef") {
        let Some(reference) = attr_value(&element.open, "ref").filter(|it| !it.trim().is_empty()) else {
            continue;
        };
        let (theme_ref, entry) = split_ref(&reference);
        if entry.trim().is_empty() {
            themes.entry(theme_ref).or_default();
        } else {
            themes.entry(theme_ref).or_default().push(entry);
        }
    }
    themes
        .into_iter()
        .map(|(theme_ref, mut entries)| {
            entries.sort();
            entries.dedup();
            UiThemeLibraryRef { theme_ref, entries }
        })
        .collect()
}

fn parse_component_templates(xml: &str, source_ref: &str) -> Vec<UiComponentTemplate> {
    let mut templates = Vec::new();
    for component in elements(xml, "Component") {
        let id = attr_value(&component.open, "id")
            .or_else(|| attr_value(&component.open, "name"))
            .unwrap_or_default();
        if id.trim().is_empty() {
            continue;
        }
        let Some(root_element) = direct_child_elements(&component.inner).into_iter().find(|child| !is_metadata_element(&child.name)) else {
            continue;
        };
        let root = parse_ui_node_element(xml, &root_element, source_ref, 0, 0)
            .unwrap_or_else(|| UiNodeRequest::new(format!("{id}.root"), UiRuntimeNodeKind::Panel));
        let required_props = attr_value(&component.open, "required_props")
            .unwrap_or_default()
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|it| !it.trim().is_empty())
            .map(str::to_owned)
            .collect();
        templates.push(UiComponentTemplate { id, source_ref: source_ref.to_owned(), required_props, root });
    }
    templates
}

fn resolve_imported_component_templates(
    state: &mut AssetsUiRuntimeState,
    libraries: &[UiComponentLibraryRef],
    warnings: &mut Vec<String>,
) -> Vec<UiComponentTemplate> {
    let mut out = Vec::new();
    for library in libraries {
        if library.library_ref.trim().is_empty() {
            continue;
        }
        let request = AssetsUiRefRequest {
            document_ref: library.library_ref.clone(),
            ..Default::default()
        };
        match load_xmlcentral(state, request) {
            Ok((xml, _, resolved)) => {
                let mut templates = parse_component_templates(&xml, &resolved.document_ref);
                if !library.entries.is_empty() {
                    templates.retain(|template| library.entries.iter().any(|entry| entry == &template.id));
                }
                warnings.push(format!(
                    ".neui component library resolved ref='{}' templates={}",
                    resolved.document_ref,
                    templates.len()
                ));
                out.extend(templates);
            }
            Err(err) => warnings.push(format!(
                ".neui component library unresolved ref='{}' err='{}'",
                library.library_ref, err
            )),
        }
    }
    out
}

fn merge_component_templates(
    imported: Vec<UiComponentTemplate>,
    local: Vec<UiComponentTemplate>,
) -> Vec<UiComponentTemplate> {
    let mut by_id = BTreeMap::<String, UiComponentTemplate>::new();
    for template in imported.into_iter().chain(local.into_iter()) {
        if template.id.trim().is_empty() {
            continue;
        }
        by_id.insert(template.id.clone(), template);
    }
    by_id.into_values().collect()
}

fn resolve_theme_token_bundle(
    state: &mut AssetsUiRuntimeState,
    libraries: &[UiThemeLibraryRef],
    fallback_ref: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<ThemeTokenBundle> {
    let mut resolved = ThemeTokenBundle::default();
    for library in libraries {
        if library.theme_ref.trim().is_empty() {
            continue;
        }
        let request = AssetsUiRefRequest {
            document_ref: library.theme_ref.clone(),
            ..Default::default()
        };
        match load_xmlcentral(state, request) {
            Ok((xml, _, resolved_ref)) => {
                let bundle = parse_theme_tokens(&xml, &resolved_ref.document_ref, &library.entries);
                if let Some(bundle) = bundle {
                    merge_theme_tokens(&mut resolved, bundle);
                    warnings.push(format!(
                        ".neui theme library resolved ref='{}' density='{}' tokens={}",
                        resolved_ref.document_ref,
                        resolved.density,
                        resolved.tokens.len()
                    ));
                } else {
                    warnings.push(format!(".neui theme library contains no Theme entry ref='{}'", resolved_ref.document_ref));
                }
            }
            Err(err) => warnings.push(format!(
                ".neui theme library unresolved ref='{}' err='{}'",
                library.theme_ref, err
            )),
        }
    }
    if resolved.theme_ref.trim().is_empty() {
        if let Some(reference) = fallback_ref.filter(|it| !it.trim().is_empty()) {
            resolved.theme_ref = reference.to_owned();
            resolved.theme_id = reference.to_owned();
            resolved.density = "normal".to_owned();
            return Some(resolved);
        }
        return None;
    }
    Some(resolved)
}

fn parse_theme_tokens(xml: &str, source_ref: &str, entries: &[String]) -> Option<ThemeTokenBundle> {
    let themes = elements(xml, "Theme");
    let selected = if entries.is_empty() {
        themes.into_iter().next()
    } else {
        themes
            .into_iter()
            .find(|theme| attr_value(&theme.open, "name").map(|name| entries.iter().any(|entry| entry == &name)).unwrap_or(false))
    }?;

    let mut bundle = ThemeTokenBundle {
        theme_ref: source_ref.to_owned(),
        theme_id: attr_value(&selected.open, "id")
            .or_else(|| attr_value(&selected.open, "theme_id"))
            .or_else(|| attr_value(&selected.open, "name"))
            .unwrap_or_else(|| source_ref.to_owned()),
        density: attr_value(&selected.open, "density").unwrap_or_else(|| "normal".to_owned()),
        ..Default::default()
    };

    for token in elements(&selected.inner, "Token") {
        let Some(name) = attr_value(&token.open, "name").filter(|it| !it.trim().is_empty()) else { continue; };
        let value = attr_value(&token.open, "value")
            .or_else(|| attr_value(&token.open, "ref"))
            .unwrap_or_default();
        insert_theme_token(&mut bundle, &name, &value);
    }
    for color in elements(&selected.inner, "Color") {
        if let (Some(name), Some(value)) = (attr_value(&color.open, "name"), attr_value(&color.open, "value")) {
            insert_theme_token(&mut bundle, &format!("color.{name}"), &value);
        }
    }
    for metric in elements(&selected.inner, "Metric") {
        if let (Some(name), Some(value)) = (attr_value(&metric.open, "name"), attr_value(&metric.open, "value")) {
            insert_theme_token(&mut bundle, &format!("metric.{name}"), &value);
        }
    }
    for font in elements(&selected.inner, "FontToken") {
        if let (Some(name), Some(value)) = (attr_value(&font.open, "name"), attr_value(&font.open, "ref").or_else(|| attr_value(&font.open, "value"))) {
            insert_theme_token(&mut bundle, &format!("font.{name}"), &value);
        }
    }
    Some(bundle)
}

fn insert_theme_token(bundle: &mut ThemeTokenBundle, name: &str, value: &str) {
    let name = name.trim().to_owned();
    let value = value.trim().to_owned();
    if name.is_empty() {
        return;
    }
    if name == "density" || name == "density.mode" {
        if !value.is_empty() { bundle.density = value.clone(); }
    }
    if let Some(color) = parse_hex_rgba(&value) {
        if name.starts_with("color.") {
            bundle.colors.insert(name.clone(), color);
        }
    }
    if name.starts_with("metric.") {
        if let Ok(number) = value.parse::<f32>() {
            bundle.metrics.insert(name.trim_start_matches("metric.").to_owned(), number);
        }
    }
    if name.starts_with("font.") && !value.is_empty() {
        bundle.font_tokens.insert(name.trim_start_matches("font.").to_owned(), value.clone());
    }
    bundle.tokens.insert(name, value);
}

fn merge_theme_tokens(target: &mut ThemeTokenBundle, source: ThemeTokenBundle) {
    if !source.theme_ref.trim().is_empty() { target.theme_ref = source.theme_ref; }
    if !source.theme_id.trim().is_empty() { target.theme_id = source.theme_id; }
    if !source.density.trim().is_empty() { target.density = source.density; }
    target.tokens.extend(source.tokens);
    target.colors.extend(source.colors);
    target.metrics.extend(source.metrics);
    target.font_tokens.extend(source.font_tokens);
}

fn parse_hex_rgba(value: &str) -> Option<[u8; 4]> {
    let hex = value.trim().trim_start_matches('#');
    let read = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
    match hex.len() {
        6 => Some([read(0..2)?, read(2..4)?, read(4..6)?, 255]),
        8 => Some([read(0..2)?, read(2..4)?, read(4..6)?, read(6..8)?]),
        _ => None,
    }
}

fn compile_surface_root(xml: &str, surface: &SurfaceInfo, source_ref: &str, style_ref: Option<&str>) -> Result<UiNodeRequest, String> {
    let layout = layout_by_name(xml, &surface.root)
        .or_else(|| first_element(xml, "Layout"))
        .ok_or_else(|| {
            let span = source_span_for_named_element(xml, "Surface", source_ref);
            format!(
                "{} entry='@{}' {}: .neui surface '{}' points to missing layout '{}'",
                source_ref,
                surface.root,
                span.display(source_ref),
                surface.name,
                surface.root
            )
        })?;

    let mut root = UiNodeRequest::new(surface.name.clone(), UiRuntimeNodeKind::Surface);
    root.component_id = UI_COMPONENT_SURFACE.to_owned();
    root.role = attr_value(&layout.open, "role").unwrap_or_else(|| "surface".to_owned());
    root.text = attr_value(&layout.open, "title").unwrap_or_else(|| surface.name.clone());
    root.visible = !bool_attr(&layout.open, "hidden");
    root.interactive = false;
    root.source_span = Some(source_span_for_open(xml, &layout.open, source_ref));
    root.style_tags.extend(["surface-root".to_owned(), format!("surface:{}", sanitize_tag(&surface.name))]);
    if !surface.kind.trim().is_empty() {
        root.style_tags.push(format!("surface-kind:{}", sanitize_tag(&surface.kind)));
    }
    root.props.insert("surface_id".to_owned(), serde_json::Value::String(surface.name.clone()));
    root.props.insert("surface_kind".to_owned(), serde_json::Value::String(surface.kind.clone()));
    root.props.insert("root_layout".to_owned(), serde_json::Value::String(surface.root.clone()));
    root.props.insert("modal".to_owned(), serde_json::Value::Bool(surface.modal));
    root.props.insert("z_order".to_owned(), serde_json::json!(surface.z_order));
    if let Some(theme) = surface.theme.as_ref().filter(|it| !it.trim().is_empty()) {
        root.props.insert("theme_ref".to_owned(), serde_json::Value::String(theme.clone()));
    }
    if let Some(style_ref) = style_ref.filter(|it| !it.trim().is_empty()) {
        root.props.insert("style_ref".to_owned(), serde_json::Value::String(style_ref.to_owned()));
    }
    root.children = parse_layout_children(xml, &layout, source_ref, 0);
    if root.children.is_empty() {
        let span = source_span_for_open(xml, &layout.open, source_ref);
        return Err(format!(
            "{} entry='@{}' {}: .neui surface '{}' layout '{}' compiled to an empty root",
            source_ref,
            surface.root,
            span.display(source_ref),
            surface.name,
            surface.root
        ));
    }
    Ok(root)
}

fn parse_layout_children(xml: &str, layout: &XmlElement, source_ref: &str, depth: usize) -> Vec<UiNodeRequest> {
    direct_child_elements(&layout.inner)
        .into_iter()
        .enumerate()
        .filter_map(|(idx, child)| parse_ui_node_element(xml, &child, source_ref, idx, depth + 1))
        .collect()
}

fn parse_ui_node_element(xml: &str, element: &XmlElement, source_ref: &str, generated_index: usize, depth: usize) -> Option<UiNodeRequest> {
    if is_metadata_element(&element.name) || depth > 48 {
        return None;
    }

    let (kind, mut implicit_tags) = kind_for_neui_tag(&element.name);
    let id = attr_value(&element.open, "id")
        .or_else(|| attr_value(&element.open, "name"))
        .unwrap_or_else(|| format!("{}.{}", sanitize_tag(&element.name), generated_index));
    let mut node = UiNodeRequest::new(id.clone(), kind);
    node.component_id = attr_value(&element.open, "component")
        .or_else(|| attr_value(&element.open, "template"))
        .unwrap_or_else(|| kind.default_component_id().to_owned());
    node.role = attr_value(&element.open, "role").unwrap_or_else(|| sanitize_tag(&element.name));
    node.source_span = Some(source_span_for_open(xml, &element.open, source_ref));
    node.text = attr_value(&element.open, "text")
        .or_else(|| attr_value(&element.open, "label"))
        .or_else(|| attr_value(&element.open, "title"))
        .or_else(|| attr_value(&element.open, "value").filter(|_| kind == UiRuntimeNodeKind::Text))
        .unwrap_or_default();
    node.value = attr_value(&element.open, "value").filter(|_| kind != UiRuntimeNodeKind::Text);
    node.detail = attr_value(&element.open, "detail").or_else(|| attr_value(&element.open, "subtitle"));
    node.icon = attr_value(&element.open, "icon").or_else(|| attr_value(&element.open, "texture"));
    node.font_token = attr_value(&element.open, "font").or_else(|| attr_value(&element.open, "font_token"));
    node.tooltip = attr_value(&element.open, "tooltip");
    node.visible = !bool_attr(&element.open, "hidden") && !matches!(attr_value(&element.open, "visible").as_deref(), Some("false") | Some("0") | Some("no"));
    node.enabled = !matches!(attr_value(&element.open, "enabled").as_deref(), Some("false") | Some("0") | Some("no"));
    node.interactive = bool_attr(&element.open, "interactive") || is_intrinsically_interactive(kind);
    node.tone = tone_from_node_attrs(&element.open);

    node.style_tags.extend(class_tags(&element.open));
    node.style_tags.push(sanitize_tag(&element.name));
    node.style_tags.append(&mut implicit_tags);
    node.style_tags.sort();
    node.style_tags.dedup();

    for (key, value) in parse_attrs(&element.open) {
        if is_structural_attr(&key) {
            continue;
        }
        node.props.insert(key, serde_json::Value::String(xml_unescape(&value)));
    }

    for (idx, child) in direct_child_elements(&element.inner).into_iter().enumerate() {
        match child.name.as_str() {
            "Bind" => {
                let binding = binding_from_element(&child);
                if node.text.trim().is_empty() && binding.property == "text" {
                    if let Some(value) = binding.fallback.as_str().filter(|it| !it.trim().is_empty()) {
                        node.text = value.to_owned();
                    }
                }
                if node.value.is_none() && binding.property == "value" {
                    if let Some(value) = binding.fallback.as_str().filter(|it| !it.trim().is_empty()) {
                        node.value = Some(value.to_owned());
                    }
                }
                node.bindings.push(binding);
            }
            "Event" => {
                let route = event_route_from_element(&child);
                if node.action_id.is_none() && route.trigger == UiNodeEventTrigger::Click && !route.action_id.trim().is_empty() {
                    node.action_id = Some(route.action_id.clone());
                    node.interactive = true;
                }
                node.events.push(route);
            }
            "Text" if matches!(kind, UiRuntimeNodeKind::Button | UiRuntimeNodeKind::Action) && attr_value(&child.open, "id").is_none() => {
                if node.text.trim().is_empty() {
                    node.text = attr_value(&child.open, "value")
                        .or_else(|| attr_value(&child.open, "text"))
                        .or_else(|| attr_value(&child.open, "label"))
                        .unwrap_or_default();
                }
            }
            _ => {
                if let Some(child_node) = parse_ui_node_element(xml, &child, source_ref, idx, depth + 1) {
                    node.children.push(child_node);
                }
            }
        }
    }

    if let Some(use_layout) = attr_value(&element.open, "use").filter(|it| !it.trim().is_empty()) {
        if let Some(layout) = layout_by_name(xml, &use_layout) {
            node.children.extend(parse_layout_children(xml, &layout, source_ref, depth + 1));
            node.props.insert("use".to_owned(), serde_json::Value::String(use_layout));
        }
    }

    if node.action_id.is_none() {
        node.action_id = attr_value(&element.open, "action")
            .or_else(|| attr_value(&element.open, "action_id"))
            .or_else(|| attr_value(&element.open, "command"));
        if node.action_id.is_some() {
            node.interactive = true;
        }
    }
    if matches!(kind, UiRuntimeNodeKind::Action) && node.action_id.is_none() {
        let value = node.value.clone().unwrap_or_else(|| id.clone());
        node.action_id = Some(format!("ui.select.{value}"));
        node.interactive = true;
    }
    if node.text.trim().is_empty() && matches!(kind, UiRuntimeNodeKind::Action | UiRuntimeNodeKind::Button) {
        node.text = id.clone();
    }

    Some(node)
}

fn layout_by_name(xml: &str, name: &str) -> Option<XmlElement> {
    elements(xml, "Layout")
        .into_iter()
        .find(|layout| attr_value(&layout.open, "name").as_deref() == Some(name))
}

fn is_metadata_element(name: &str) -> bool {
    matches!(
        name,
        "Entries" | "Entry" | "Surface" | "Dependencies" | "ThemeRef" | "ComponentRef" | "TextureRef" | "FontRef"
            | "SoundRef" | "BindingGraph" | "StateSource" | "Bind" | "ActionMap" | "Action" | "Event" | "Payload" | "Slot"
            | "UiNodeNavigationDocument" | "Page" | "Footer" | "Line" | "NavLeft" | "NavRight" | "Back"
    )
}

fn kind_for_neui_tag(name: &str) -> (UiRuntimeNodeKind, Vec<String>) {
    let normalized = sanitize_tag(name).replace('-', "");
    match normalized.as_str() {
        "surface" => (UiRuntimeNodeKind::Surface, vec![UI_COMPONENT_SURFACE.to_owned()]),
        "panel" | "card" | "statuscard" | "metriccard" | "warningcard" | "plugincard" | "propertycard" => {
            (UiRuntimeNodeKind::Panel, vec![sanitize_tag(name)])
        }
        "stack" => (UiRuntimeNodeKind::Stack, vec![UI_COMPONENT_STACK.to_owned()]),
        "row" => (UiRuntimeNodeKind::Row, vec![UI_COMPONENT_ROW.to_owned()]),
        "column" | "col" => (UiRuntimeNodeKind::Column, vec!["column".to_owned()]),
        "grid" => (UiRuntimeNodeKind::Grid, vec![UI_COMPONENT_GRID.to_owned()]),
        "text" | "label" => (UiRuntimeNodeKind::Text, vec![UI_COMPONENT_TEXT.to_owned()]),
        "button" => (UiRuntimeNodeKind::Button, vec![UI_COMPONENT_BUTTON.to_owned()]),
        "action" | "option" | "item" | "selectitem" | "dropdownitem" | "menuitem" => {
            (UiRuntimeNodeKind::Action, vec![UI_COMPONENT_ACTION.to_owned(), "select-option".to_owned(), "option".to_owned()])
        }
        "input" | "textinput" | "field" | "search" => (UiRuntimeNodeKind::Input, vec![UI_COMPONENT_INPUT.to_owned()]),
        "checkbox" | "check" => (UiRuntimeNodeKind::Checkbox, vec![UI_COMPONENT_CHECKBOX.to_owned()]),
        "toggle" | "switch" => (UiRuntimeNodeKind::Toggle, vec![UI_COMPONENT_TOGGLE.to_owned()]),
        "slider" | "progress" | "progressbar" => (UiRuntimeNodeKind::Slider, vec![UI_COMPONENT_SLIDER.to_owned(), normalized]),
        "scrollbar" => (UiRuntimeNodeKind::ScrollBar, vec![UI_COMPONENT_SCROLL_BAR.to_owned()]),
        "select" | "dropdown" | "combobox" => (UiRuntimeNodeKind::Select, vec![UI_COMPONENT_SELECT.to_owned()]),
        "separator" | "divider" => (UiRuntimeNodeKind::Separator, vec![UI_COMPONENT_SEPARATOR.to_owned()]),
        "list" | "propertygrid" => (UiRuntimeNodeKind::List, vec![UI_COMPONENT_LIST.to_owned(), sanitize_tag(name)]),
        "tree" => (UiRuntimeNodeKind::Tree, vec![UI_COMPONENT_TREE.to_owned()]),
        "split" | "splitter" => (UiRuntimeNodeKind::Split, vec!["split".to_owned()]),
        "viewport" => (UiRuntimeNodeKind::Viewport, vec![UI_COMPONENT_VIEWPORT.to_owned()]),
        "image" | "texture" | "externaltexture" | "icon" => (UiRuntimeNodeKind::ExternalTexture, vec![UI_COMPONENT_EXTERNAL_TEXTURE.to_owned()]),
        "spacer" => (UiRuntimeNodeKind::Spacer, vec![UI_COMPONENT_SPACER.to_owned()]),
        _ => (UiRuntimeNodeKind::Panel, vec!["custom".to_owned(), sanitize_tag(name)]),
    }
}

fn is_intrinsically_interactive(kind: UiRuntimeNodeKind) -> bool {
    matches!(
        kind,
        UiRuntimeNodeKind::Action
            | UiRuntimeNodeKind::Button
            | UiRuntimeNodeKind::Input
            | UiRuntimeNodeKind::Checkbox
            | UiRuntimeNodeKind::Toggle
            | UiRuntimeNodeKind::Slider
            | UiRuntimeNodeKind::ScrollBar
            | UiRuntimeNodeKind::Select
            | UiRuntimeNodeKind::List
            | UiRuntimeNodeKind::Tree
            | UiRuntimeNodeKind::Split
            | UiRuntimeNodeKind::Viewport
    )
}

fn binding_from_element(element: &XmlElement) -> UiNodeBindingRequest {
    let source = attr_value(&element.open, "source").unwrap_or_default();
    let (source_id, path) = if let Some((source_id, path)) = source.split_once('.') {
        (source_id.to_owned(), path.to_owned())
    } else {
        (String::new(), source.clone())
    };
    UiNodeBindingRequest {
        property: attr_value(&element.open, "property").unwrap_or_else(|| "value".to_owned()),
        source: source_id,
        path,
        mode: attr_value(&element.open, "mode").unwrap_or_else(|| "read".to_owned()),
        fallback: attr_value(&element.open, "fallback").map(json_value_from_attr).unwrap_or(serde_json::Value::Null),
    }
}

fn event_route_from_element(element: &XmlElement) -> UiNodeEventRoute {
    let mut payload = serde_json::Map::new();
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(xml_unescape(&value)));
        }
    }
    UiNodeEventRoute {
        trigger: trigger_from_attr(attr_value(&element.open, "trigger").as_deref()),
        action_id: attr_value(&element.open, "action")
            .or_else(|| attr_value(&element.open, "action_id"))
            .or_else(|| attr_value(&element.open, "id"))
            .unwrap_or_default(),
        target_gateway: attr_value(&element.open, "target").unwrap_or_else(|| newengine_ui_api::ENGINE_UI_SERVICE_ID.to_owned()),
        method: attr_value(&element.open, "method").unwrap_or_else(|| newengine_ui_api::UI_SERVICE_METHOD_DISPATCH_ACTION_V1.to_owned()),
        payload: serde_json::Value::Object(payload),
    }
}

fn trigger_from_attr(value: Option<&str>) -> UiNodeEventTrigger {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "hover_enter" | "mouseenter" => UiNodeEventTrigger::HoverEnter,
        "hover_exit" | "mouseleave" => UiNodeEventTrigger::HoverExit,
        "press" | "pointer_down" => UiNodeEventTrigger::Press,
        "release" | "pointer_up" => UiNodeEventTrigger::Release,
        "double_click" | "dblclick" => UiNodeEventTrigger::DoubleClick,
        "focus" => UiNodeEventTrigger::Focus,
        "blur" => UiNodeEventTrigger::Blur,
        "value_changed" | "change" => UiNodeEventTrigger::ValueChanged,
        "drag_start" => UiNodeEventTrigger::DragStart,
        "drag_move" => UiNodeEventTrigger::DragMove,
        "drag_end" => UiNodeEventTrigger::DragEnd,
        "context_menu" => UiNodeEventTrigger::ContextMenu,
        _ => UiNodeEventTrigger::Click,
    }
}

fn tone_from_node_attrs(open: &str) -> UiNodeTone {
    let tone = attr_value(open, "tone").unwrap_or_default().to_ascii_lowercase();
    let classes = attr_value(open, "class").unwrap_or_default().to_ascii_lowercase();
    if tone == "danger" || classes.contains("danger") {
        UiNodeTone::Danger
    } else if tone == "accent" || tone == "primary" || classes.contains("primary") || classes.contains("accent") {
        UiNodeTone::Accent
    } else if tone == "disabled" || classes.contains("disabled") {
        UiNodeTone::Disabled
    } else {
        UiNodeTone::Normal
    }
}

fn class_tags(open: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for token in attr_value(open, "class").unwrap_or_default().split_whitespace() {
        let tag = sanitize_tag(token);
        if tag.is_empty() {
            continue;
        }
        tags.push(tag.clone());
        for prefix in ["button-", "ui-", "aurelia-", "dark-", "light-"] {
            if let Some(rest) = tag.strip_prefix(prefix).filter(|it| !it.is_empty()) {
                tags.push(rest.to_owned());
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

fn sanitize_tag(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

fn source_span_for_named_element(xml: &str, name: &str, source_ref: &str) -> UiSourceSpan {
    first_element(xml, name)
        .map(|element| source_span_for_open(xml, &element.open, source_ref))
        .unwrap_or_else(|| source_span_for_offset(xml, 0, source_ref))
}

fn source_span_for_open(xml: &str, open: &str, source_ref: &str) -> UiSourceSpan {
    let offset = xml.find(open).unwrap_or(0);
    source_span_for_offset(xml, offset, source_ref)
}

fn source_span_for_offset(xml: &str, offset: usize, source_ref: &str) -> UiSourceSpan {
    let mut line = 1u32;
    let mut column = 1u32;
    for ch in xml[..offset.min(xml.len())].chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }
    UiSourceSpan { source_ref: source_ref.to_owned(), line, column }
}

fn json_value_from_attr(value: String) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        serde_json::Value::Bool(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        serde_json::Value::Bool(false)
    } else if let Ok(number) = trimmed.parse::<i64>() {
        serde_json::Value::Number(number.into())
    } else if let Ok(number) = trimmed.parse::<f64>() {
        serde_json::Number::from_f64(number).map(serde_json::Value::Number).unwrap_or_else(|| serde_json::Value::String(value))
    } else {
        serde_json::Value::String(value)
    }
}

fn is_structural_attr(key: &str) -> bool {
    matches!(
        key,
        "id" | "name" | "class" | "role" | "text" | "label" | "title" | "detail" | "subtitle" | "value" | "icon" | "texture"
            | "font" | "font_token" | "tooltip" | "hidden" | "visible" | "enabled" | "interactive" | "tone" | "action"
            | "action_id" | "command" | "use" | "component" | "template"
    )
}


fn derive_navigation_document_from_surface_layout(xml: &str, surface: &SurfaceInfo) -> Result<Option<UiNodeNavigationDocument>, String> {
    let Some(layout) = elements(xml, "Layout")
        .into_iter()
        .find(|layout| attr_value(&layout.open, "name").as_deref() == Some(surface.root.as_str()))
        .or_else(|| first_element(xml, "Layout"))
    else {
        return Ok(None);
    };

    let buttons = elements(&layout.inner, "Button");
    if buttons.is_empty() {
        return Ok(None);
    }

    let routes = action_map_routes(xml);
    let mut items = Vec::new();
    for (idx, button) in buttons.into_iter().enumerate() {
        let id = attr_value(&button.open, "id").unwrap_or_else(|| format!("ui.item.{idx}"));
        let label = attr_value(&button.open, "label")
            .or_else(|| first_element(&button.inner, "Text").and_then(|text| attr_value(&text.open, "value")))
            .unwrap_or_else(|| id.clone());
        if label.trim().is_empty() {
            continue;
        }

        let action_id = first_element(&button.inner, "Event")
            .and_then(|event| attr_value(&event.open, "action"));
        let action = action_id
            .as_deref()
            .and_then(|id| routes.get(id).cloned())
            .or_else(|| action_id.as_deref().map(default_route_for_action_id));
        let class = attr_value(&button.open, "class").unwrap_or_default().to_ascii_lowercase();
        let tone = if class.contains("primary") || idx == 0 {
            UiNodeNavigationTone::Accent
        } else {
            UiNodeNavigationTone::Normal
        };

        items.push(UiNodeNavigationItem {
            id,
            label,
            value: None,
            detail: None,
            emphasized: class.contains("primary"),
            tone,
            dynamic_value: None,
            action,
            nav_left: None,
            nav_right: None,
        });
    }

    if items.is_empty() {
        return Ok(None);
    }

    let title = first_text_with_class(&layout.inner, "title")
        .or_else(|| attr_value(&layout.open, "title"))
        .unwrap_or_else(|| surface.name.clone());

    let doc = UiNodeNavigationDocument {
        id: "engine.ui.primary".to_owned(),
        version: 1,
        surface_id: surface.name.clone(),
        root_page: "root".to_owned(),
        title,
        subtitle: "Declarative .neui layout projected as a navigation document".to_owned(),
        footer_lines: vec![
            "ESC / START - Close menu".to_owned(),
            "ARROWS / DPAD - Navigate".to_owned(),
            "ENTER / A - Confirm".to_owned(),
        ],
        pages: vec![UiNodeNavigationPage {
            id: "root".to_owned(),
            title: "Main Menu".to_owned(),
            subtitle: String::new(),
            parent_page: None,
            footer_lines: Vec::new(),
            items,
            back_route: Some(UiNodeActionRoute {
                id: "ui.close".to_owned(),
                source: "engine.assets.ui".to_owned(),
                target: "UiNodeNavigationRuntime".to_owned(),
                event: "ui.close".to_owned(),
                payload: BTreeMap::new(),
                transition: Some(UiNodeTransition::close()),
                feedback: None,
                audio: Some("ui.close".to_owned()),
            }),
        }],
    }
    .canonicalized();
    doc.validate()?;
    Ok(Some(doc))
}

fn action_map_routes(xml: &str) -> BTreeMap<String, UiNodeActionRoute> {
    let mut out = BTreeMap::new();
    for action in elements(xml, "Action") {
        let Some(id) = attr_value(&action.open, "id") else {
            continue;
        };
        out.insert(id.clone(), route_from_action_map_element(&id, &action));
    }
    out
}

fn route_from_action_map_element(id: &str, element: &XmlElement) -> UiNodeActionRoute {
    let target = attr_value(&element.open, "target").unwrap_or_else(|| "UiNodeNavigationRuntime".to_owned());
    let command = attr_value(&element.open, "command")
        .or_else(|| attr_value(&element.open, "event"))
        .unwrap_or_else(|| "ui.activate".to_owned());
    let mut payload = BTreeMap::new();
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(value));
        }
    }
    let transition = match command.as_str() {
        "ui.close" | "menu.close" | "engine.ui.close" => Some(UiNodeTransition::close()),
        "menu.open_page" | "ui.open_page" => payload
            .get("page")
            .and_then(serde_json::Value::as_str)
            .map(UiNodeTransition::open_page),
        _ => None,
    };
    UiNodeActionRoute {
        id: id.to_owned(),
        source: "engine.assets.ui".to_owned(),
        target,
        event: command,
        payload,
        transition,
        feedback: None,
        audio: Some("ui.select".to_owned()),
    }
}

fn default_route_for_action_id(id: &str) -> UiNodeActionRoute {
    UiNodeActionRoute {
        id: id.to_owned(),
        source: "engine.assets.ui".to_owned(),
        target: "UiNodeNavigationRuntime".to_owned(),
        event: id.to_owned(),
        payload: BTreeMap::new(),
        transition: None,
        feedback: None,
        audio: Some("ui.select".to_owned()),
    }
}

fn first_text_with_class(xml: &str, class_name: &str) -> Option<String> {
    elements(xml, "Text")
        .into_iter()
        .find(|text| {
            attr_value(&text.open, "class")
                .map(|class| class.split_whitespace().any(|token| token == class_name))
                .unwrap_or(false)
        })
        .and_then(|text| attr_value(&text.open, "value"))
        .filter(|value| !value.trim().is_empty())
}

fn parse_navigation_document(xml: &str) -> Result<Option<UiNodeNavigationDocument>, String> {
    let Some(navigation) = first_element(xml, "UiNodeNavigationDocument") else {
        return Ok(None);
    };
    let mut doc = UiNodeNavigationDocument {
        id: attr_value(&navigation.open, "id").unwrap_or_else(|| "engine.ui.primary".to_owned()),
        version: attr_value(&navigation.open, "version").and_then(|v| v.parse().ok()).unwrap_or(1),
        surface_id: attr_value(&navigation.open, "surface_id").or_else(|| attr_value(&navigation.open, "surface")).unwrap_or_else(|| "engine.ui.primary".to_owned()),
        root_page: attr_value(&navigation.open, "root_page").unwrap_or_else(|| "root".to_owned()),
        title: attr_value(&navigation.open, "title").unwrap_or_default(),
        subtitle: attr_value(&navigation.open, "subtitle").unwrap_or_default(),
        footer_lines: Vec::new(),
        pages: Vec::new(),
    };

    if let Some(footer) = first_element(&navigation.inner, "Footer") {
        for line in elements(&footer.inner, "Line") {
            if let Some(value) = attr_value(&line.open, "value") {
                if !value.trim().is_empty() {
                    doc.footer_lines.push(value);
                }
            }
        }
    }

    for page_element in elements(&navigation.inner, "Page") {
        let mut page = UiNodeNavigationPage {
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
            let item = UiNodeNavigationItem {
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

fn first_route_element(xml: &str, name: &str) -> Option<UiNodeActionRoute> {
    let element = first_element(xml, name)?;
    Some(route_from_element(&element))
}

fn route_from_element(element: &XmlElement) -> UiNodeActionRoute {
    let mut payload = BTreeMap::new();
    if let Some(page) = attr_value(&element.open, "page") {
        payload.insert("page".to_owned(), serde_json::Value::String(page));
    }
    if let Some(payload_element) = first_element(&element.inner, "Payload") {
        for (key, value) in parse_attrs(&payload_element.open) {
            payload.insert(key, serde_json::Value::String(value));
        }
    }
    UiNodeActionRoute {
        id: attr_value(&element.open, "id").unwrap_or_default(),
        source: attr_value(&element.open, "source").unwrap_or_default(),
        target: attr_value(&element.open, "target").unwrap_or_else(|| "UiNodeNavigationRuntime".to_owned()),
        event: attr_value(&element.open, "event").unwrap_or_else(|| event_from_route_tag(&element.name).to_owned()),
        payload,
        transition: transition_from_attrs(&element.open),
        feedback: first_element(&element.inner, "Feedback").map(|feedback| UiNodeFeedbackEvent {
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
        "Back" => "ui.back",
        "NavLeft" => "ui.nav_left",
        "NavRight" => "ui.nav_right",
        _ => "ui.activate",
    }
}

fn transition_from_attrs(open: &str) -> Option<UiNodeTransition> {
    match attr_value(open, "transition").unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "close" => Some(UiNodeTransition::close()),
        "open_page" => attr_value(open, "page").map(UiNodeTransition::open_page),
        "back" => Some(UiNodeTransition { kind: UiNodeTransitionKind::Back, page: None, reset_selection: true }),
        "none" | "" => None,
        _ => None,
    }
}

fn tone_from_attr(value: Option<&str>) -> UiNodeNavigationTone {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "accent" => UiNodeNavigationTone::Accent,
        "danger" => UiNodeNavigationTone::Danger,
        "disabled" => UiNodeNavigationTone::Disabled,
        _ => UiNodeNavigationTone::Normal,
    }
}

fn feedback_severity_from_attr(value: Option<&str>) -> UiNodeFeedbackSeverity {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "success" => UiNodeFeedbackSeverity::Success,
        "warning" => UiNodeFeedbackSeverity::Warning,
        "danger" | "error" => UiNodeFeedbackSeverity::Danger,
        _ => UiNodeFeedbackSeverity::Info,
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

fn direct_child_elements(xml: &str) -> Vec<XmlElement> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(start_rel) = xml[offset..].find('<') {
        let start = offset + start_rel;
        let Some(next) = xml.as_bytes().get(start + 1).copied() else { break; };
        if matches!(next, b'/' | b'!' | b'?') {
            offset = xml[start..].find('>').map(|end| start + end + 1).unwrap_or(xml.len());
            continue;
        }
        let Some(open_end_rel) = xml[start..].find('>') else { break; };
        let open_end = start + open_end_rel;
        let open = &xml[start..=open_end];
        let Some(name) = element_name_from_open(open) else {
            offset = open_end + 1;
            continue;
        };
        let self_closing = open.trim_end().ends_with("/>");
        if self_closing {
            out.push(XmlElement { name, open: open.to_owned(), inner: String::new() });
            offset = open_end + 1;
            continue;
        }
        let Some((close_start, close_end)) = matching_close_tag(xml, &name, open_end + 1) else { break; };
        out.push(XmlElement {
            name,
            open: open.to_owned(),
            inner: xml[open_end + 1..close_start].to_owned(),
        });
        offset = close_end;
    }
    out
}

fn element_name_from_open(open: &str) -> Option<String> {
    let rest = open.trim_start().strip_prefix('<')?.trim_start();
    let name_end = rest.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    Some(rest[..name_end].to_owned())
}

fn matching_close_tag(xml: &str, name: &str, from: usize) -> Option<(usize, usize)> {
    let open_token = format!("<{}", name);
    let close_token = format!("</{}>", name);
    let mut depth = 1usize;
    let mut offset = from;
    loop {
        let next_open = xml[offset..].find(&open_token).map(|pos| offset + pos);
        let next_close = xml[offset..].find(&close_token).map(|pos| offset + pos);
        match (next_open, next_close) {
            (Some(open_pos), Some(close_pos)) if open_pos < close_pos => {
                let next = xml.as_bytes().get(open_pos + open_token.len()).copied();
                if matches!(next, Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')) {
                    let Some(open_end_rel) = xml[open_pos..].find('>') else { return None; };
                    let open_end = open_pos + open_end_rel;
                    if !xml[open_pos..=open_end].trim_end().ends_with("/>") {
                        depth += 1;
                    }
                    offset = open_end + 1;
                } else {
                    offset = open_pos + open_token.len();
                }
            }
            (_, Some(close_pos)) => {
                depth = depth.saturating_sub(1);
                let close_end = close_pos + close_token.len();
                if depth == 0 {
                    return Some((close_pos, close_end));
                }
                offset = close_end;
            }
            _ => return None,
        }
    }
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
        let doc = derive_navigation_document_from_surface_layout(xml, &surface).unwrap().unwrap();
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

        let root = compile_surface_root(xml, &surface, "assets/ui/engine/main_menu.neui@surface", surface.theme.as_deref()).unwrap();
        assert_eq!(root.kind, UiRuntimeNodeKind::Surface);
        assert_eq!(root.children.len(), 1);
        let panel = &root.children[0];
        assert_eq!(panel.id, "main.root");
        let button = panel.children.iter().find(|node| node.id == "main.start").unwrap();
        assert_eq!(button.action_id.as_deref(), Some("game.start"));
        assert!(button.interactive);
        let select = panel.children.iter().find(|node| node.id == "graphics.quality").unwrap();
        assert!(select.interactive);
        assert_eq!(select.children[0].action_id.as_deref(), Some("ui.select.high"));
    }
}
