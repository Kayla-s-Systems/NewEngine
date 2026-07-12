#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.definitions` runtime service.
//!
//! `.ytyp` ownership lives here. The service uses `engine.assets` only as the
//! VFS/raw-bytes owner and returns single-asset Properties DTOs to tools,
//! scene/map placement loaders and the asset graph resolver.
use std::collections::{BTreeMap, BTreeSet};

use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    definitions_method, stable_hash_from_text, AssetDecodeRequest, AssetDependencyRecordV1,
    AssetReference, ASSET_LIST_FILE_BODY_OUTPUT, DEFINITIONS_BACKEND_CAPABILITY_ID,
    DEFINITIONS_RUNTIME_CONTRACT, DEFINITIONS_SERVICE_ID, DEFINITIONS_SERVICE_METHODS,
    ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, ENGINE_ASSETS_GRAPH_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
};
use newengine_authored_xml as authored_xml;
use newengine_model_domain_api::{MaterialBindingRef, MeshRenderOptions, MeshShadowPolicy};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

mod parsing;
mod projection;

use parsing::*;
use projection::*;

pub const DEFINITIONS_GATEWAY_OWNER: &str = "newengine-definitions-runtime.engine-runtime-provider";

#[derive(Clone)]
pub struct DefinitionsRuntimeState {
    client: AssetServiceClient,
}

impl DefinitionsRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self { client }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionsServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub methods: &'static [&'static str],
    pub ownership_policy: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRefRequest {
    pub definition_ref: String,
    pub source: String,
    pub entry: Option<String>,
}

impl Default for DefinitionRefRequest {
    #[inline]
    fn default() -> Self {
        Self {
            definition_ref: String::new(),
            source: String::new(),
            entry: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionManifestRequest {
    pub source: String,
    pub definition_ref: String,
}

impl Default for DefinitionManifestRequest {
    #[inline]
    fn default() -> Self {
        Self {
            source: String::new(),
            definition_ref: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct RawDefinitionEntryV1 {
    #[serde(alias = "id", alias = "asset_name", alias = "assetName")]
    name: String,
    #[serde(alias = "stableHash")]
    stable_hash: u64,
    #[serde(alias = "entryKind")]
    entry_kind: String,
    kind: String,
    schema: String,
    target: Option<serde_json::Value>,
    dependencies: Vec<AssetDependencyRecordV1>,
    namespaces: BTreeMap<String, serde_json::Value>,
    metadata: BTreeMap<String, serde_json::Value>,
    #[serde(alias = "materialBindings")]
    material_bindings: Vec<MaterialBindingRef>,
    #[serde(alias = "semanticTags")]
    semantic_tags: Vec<String>,
    #[serde(alias = "domainTags")]
    domain_tags: Vec<String>,
    #[serde(alias = "sideEffects")]
    side_effects: Vec<DefinitionSideEffectV1>,
    flags: u32,
}

impl Default for RawDefinitionEntryV1 {
    fn default() -> Self {
        Self {
            name: String::new(),
            stable_hash: 0,
            entry_kind: "archetype_definition".to_owned(),
            kind: String::new(),
            schema: String::new(),
            target: None,
            dependencies: Vec::new(),
            namespaces: BTreeMap::new(),
            metadata: BTreeMap::new(),
            material_bindings: Vec::new(),
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            side_effects: Vec::new(),
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionIdentityV1 {
    pub name: String,
    pub source: String,
    pub definition_ref: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRefsV1 {
    pub drawable_refs: Vec<String>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub uv_layout_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub collision_refs: Vec<String>,
    pub ai_refs: Vec<String>,
    pub streaming_refs: Vec<String>,
    pub editor_refs: Vec<String>,
    pub other_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionSideEffectV1 {
    pub domain: String,
    pub effect: String,
    pub target: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelExplanationV1 {
    pub schema: String,
    pub source: String,
    pub model_ref: Option<String>,
    pub drawable_ref: Option<String>,
    pub material_bindings: Vec<MaterialBindingRef>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub uv_layout_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub collision_refs: Vec<String>,
    pub render_options: MeshRenderOptions,
    pub collision_policy: String,
    pub uv_policy: String,
    pub physics_policy: String,
    pub lod_policy: String,
    pub streaming_policy: String,
    pub explanation: String,
}

impl Default for ModelExplanationV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.ytyp.model_explanation.v1".to_owned(),
            source: String::new(),
            model_ref: None,
            drawable_ref: None,
            material_bindings: Vec::new(),
            material_refs: Vec::new(),
            texture_refs: Vec::new(),
            uv_layout_refs: Vec::new(),
            physics_refs: Vec::new(),
            collision_refs: Vec::new(),
            render_options: MeshRenderOptions::world_opaque(),
            collision_policy: "unspecified".to_owned(),
            uv_policy: "authored".to_owned(),
            physics_policy: "unspecified".to_owned(),
            lod_policy: "unspecified".to_owned(),
            streaming_policy: "unspecified".to_owned(),
            explanation: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntryV1 {
    pub schema: String,
    pub identity: DefinitionIdentityV1,
    pub kind: String,
    pub stable_hash: u64,
    pub semantic_tags: Vec<String>,
    pub domain_tags: Vec<String>,
    pub refs: DefinitionRefsV1,
    pub model_explanation: ModelExplanationV1,
    pub side_effects: Vec<DefinitionSideEffectV1>,
    pub arbitrary_metadata: BTreeMap<String, serde_json::Value>,
    pub warnings: Vec<String>,
}

impl Default for DefinitionEntryV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.assets.definitions.entry.v1".to_owned(),
            identity: DefinitionIdentityV1::default(),
            kind: "archetype_definition".to_owned(),
            stable_hash: 0,
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            refs: DefinitionRefsV1::default(),
            model_explanation: ModelExplanationV1::default(),
            side_effects: Vec::new(),
            arbitrary_metadata: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionManifestEntryV1 {
    pub name: String,
    pub stable_hash: u64,
    pub kind: String,
    pub semantic_tags: Vec<String>,
    pub domain_tags: Vec<String>,
    pub definition_ref: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionManifestV1 {
    pub schema: &'static str,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub source: String,
    pub status: &'static str,
    pub entries: Vec<DefinitionManifestEntryV1>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionRefResolutionV1 {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub definition_ref: String,
    pub refs: DefinitionRefsV1,
    pub flattened_refs: Vec<String>,
    pub resolver: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionValidationV1 {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub definition_ref: String,
    pub code: &'static str,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct StableDiagnostic {
    ok: bool,
    code: &'static str,
    message: String,
    gateway: &'static str,
    byte_owner: &'static str,
    semantic_owner: &'static str,
}

pub fn definitions_service_info() -> DefinitionsServiceInfo {
    DefinitionsServiceInfo {
        id: DEFINITIONS_SERVICE_ID,
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        provider: "StarVaultDefinitionsRuntimeProvider",
        contract: DEFINITIONS_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        methods: DEFINITIONS_SERVICE_METHODS,
        ownership_policy: ".ytyp Definition Entry metadata is owned by engine.assets.definitions; scene/model may consume refs but never decode or own .ytyp; AssetManager only exposes NEF8 envelope/body bytes",
    }
}

fn normalize_logical_ref(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Normalize separators and collapse duplicate slashes in one pass. The
    // previous replace/contains loop repeatedly rescanned and reallocated the
    // whole path for malformed input with many separators.
    let mut normalized = String::with_capacity(trimmed.len());
    let mut previous_was_slash = false;
    for character in trimmed.chars() {
        let character = if character == '\\' { '/' } else { character };
        if character == '/' {
            if previous_was_slash {
                continue;
            }
            previous_was_slash = true;
        } else {
            previous_was_slash = false;
        }
        normalized.push(character);
    }

    let mut start = 0usize;
    while normalized[start..].starts_with("./") {
        start += 2;
    }
    while normalized[start..].starts_with('/') {
        start += 1;
    }
    if start == 0 {
        normalized
    } else {
        normalized[start..].to_owned()
    }
}
fn definition_ref_from_request(request: &DefinitionRefRequest) -> Result<String, String> {
    if !request.definition_ref.trim().is_empty() {
        return Ok(normalize_logical_ref(&request.definition_ref));
    }
    let source = normalize_logical_ref(&request.source);
    if source.is_empty() {
        return Err(
            "assets.definitions.entry_v1 requires definition_ref='definitions/foo.ytyp' or source='definitions/foo.ytyp'"
                .to_owned(),
        );
    }
    if let Some(entry) = request
        .entry
        .as_deref()
        .map(str::trim)
        .filter(|it| !it.is_empty())
    {
        Ok(format!("{source}@{entry}"))
    } else {
        Ok(source)
    }
}

fn parse_definition_ref(request: &DefinitionRefRequest) -> Result<AssetReference, String> {
    let raw = definition_ref_from_request(request)?;
    newengine_assets_api::require_asset_reference_extension(&raw, &["ytyp"], false)
        .map_err(|e| e.to_string())
}

fn manifest_source_from_request(request: &DefinitionManifestRequest) -> Result<String, String> {
    let raw = if !request.source.trim().is_empty() {
        request.source.trim()
    } else if !request.definition_ref.trim().is_empty() {
        request
            .definition_ref
            .split('@')
            .next()
            .unwrap_or(request.definition_ref.trim())
    } else {
        return Err("assets.definitions.manifest_v1 requires source='definitions/foo.ytyp' or definition_ref='definitions/foo.ytyp'".to_owned());
    };
    let normalized = normalize_logical_ref(raw);
    let reference =
        newengine_assets_api::require_asset_reference_extension(&normalized, &["ytyp"], false)
            .map_err(|e| e.to_string())?;
    Ok(reference.logical_path)
}

fn ref_request_from_payload(payload: &[u8], method: &str) -> Result<DefinitionRefRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires definition_ref='.ytyp'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionRefRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionRefRequest {
            definition_ref: trimmed.trim_matches('"').to_owned(),
            ..Default::default()
        })
    }
}

fn manifest_request_from_payload(
    payload: &[u8],
    method: &str,
) -> Result<DefinitionManifestRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires source='definitions/foo.ytyp'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionManifestRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionManifestRequest {
            source: trimmed.trim_matches('"').to_owned(),
            ..Default::default()
        })
    }
}

fn load_ytyp_semantic_body(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<(Vec<u8>, Vec<String>), String> {
    match state.client.raw_bytes_v1(source) {
        Ok(body) if body.get(0..4) != Some(&newengine_assets_api::LIST_FILE_MAGIC_NEF8[..]) => {
            Ok((
                body,
                vec![
                    ".ytyp loose authoring body read through engine.assets raw_bytes_v1".to_owned(),
                ],
            ))
        }
        Ok(_nef8_envelope) => {
            let request = AssetDecodeRequest {
                logical_path: source.to_owned(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
            };
            state
                .client
                .decode_v1(&request)
                .map(|body| {
                    (
                        body,
                        vec![".ytyp NEF8 ListFile body decoded through engine.assets".to_owned()],
                    )
                })
                .map_err(|decode_error| {
                    format!("engine.assets.definitions: .ytyp NEF8 source requires asset.decode_v1 source='{source}' err='{decode_error}'")
                })
        }
        Err(read_error) => {
            let request = AssetDecodeRequest {
                logical_path: source.to_owned(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
            };
            state
                .client
                .decode_v1(&request)
                .map(|body| {
                    (
                        body,
                        vec![".ytyp body decoded through engine.assets after raw_bytes_v1 miss".to_owned()],
                    )
                })
                .map_err(|decode_error| {
                    format!("engine.assets.definitions: .ytyp unavailable source='{source}' read_err='{read_error}' decode_err='{decode_error}'")
                })
        }
    }
}

fn load_properties_body(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let (body, mut warnings) = load_ytyp_semantic_body(state, source)?;
    if authored_xml::body_is_xml(&body) {
        let (entries, mut parse_warnings) = parse_ytyp_xml_document(source, &body)?;
        warnings.append(&mut parse_warnings);
        warnings
            .push(".ytyp loose XML authoring body adapted into archetype metadata DTO".to_owned());
        return Ok((entries, warnings));
    }
    let (entries, mut parse_warnings) = parse_ytyp_json_document(source, &body)?;
    warnings.append(&mut parse_warnings);
    warnings.push(".ytyp semantic body parsed as archetype metadata DTO".to_owned());
    Ok((entries, warnings))
}

fn load_manifest(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<DefinitionManifestV1, String> {
    let (raw_entries, warnings) = load_properties_body(state, source)?;
    let mut entries = Vec::with_capacity(raw_entries.len());
    for raw in raw_entries {
        let name = raw.name.trim().to_owned();
        if name.is_empty() {
            continue;
        }
        let (semantic_tags, domain_tags) = collect_tags(&raw);
        entries.push(DefinitionManifestEntryV1 {
            stable_hash: effective_hash(&raw),
            kind: effective_kind(&raw),
            definition_ref: format!("{source}@{name}"),
            name,
            semantic_tags,
            domain_tags,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.stable_hash.cmp(&b.stable_hash))
    });
    Ok(DefinitionManifestV1 {
        schema: "newengine.assets.definitions.manifest.v1",
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        source: source.to_owned(),
        status: "definition_dictionary_manifest",
        entries,
        warnings,
    })
}

fn ytyp_sidecar_source(source: &str, entry_selector: &str) -> Option<String> {
    let entry = entry_selector.trim();
    if entry.is_empty() {
        return None;
    }
    let source = source.trim().replace('\\', "/");
    let (dir, _) = source.rsplit_once('/')?;
    let candidate = format!("{dir}/{entry}.ytyp");
    (candidate != source).then_some(candidate)
}

fn load_entry_from_source(
    state: &DefinitionsRuntimeState,
    source: &str,
    entry_selector: &str,
) -> Result<DefinitionEntryV1, String> {
    let (raw_entries, warnings) = load_properties_body(state, source)?;
    for raw in raw_entries {
        if entry_selector.trim().is_empty() || entry_matches(&raw, entry_selector) {
            return build_entry(source, raw, &warnings);
        }
    }
    Err(format!(
        "engine.assets.definitions: Definition Entry not found source='{}' selector='{}'",
        source, entry_selector
    ))
}

fn load_entry(
    state: &DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<DefinitionEntryV1, String> {
    let reference = parse_definition_ref(&request)?;
    let entry_selector = reference.entry.as_deref().unwrap_or_default().to_owned();
    if let Some(sidecar_source) = ytyp_sidecar_source(&reference.logical_path, &entry_selector) {
        match load_entry_from_source(state, &sidecar_source, &entry_selector) {
            Ok(mut entry) => {
                entry.identity.definition_ref = reference.canonical.clone();
                entry.warnings.push(format!(
                    ".ytyp Definition Entry resolved through sidecar source='{sidecar_source}' canonical_ref='{}'",
                    reference.canonical
                ));
                return Ok(entry);
            }
            Err(sidecar_error) => {
                let primary =
                    load_entry_from_source(state, &reference.logical_path, &entry_selector);
                return primary.map_err(|primary_error| {
                    format!(
                        "engine.assets.definitions: Definition Entry not found ref='{}' sidecar='{}' sidecar_err='{}' primary='{}'",
                        reference.canonical, sidecar_source, sidecar_error, primary_error
                    )
                });
            }
        }
    }
    load_entry_from_source(state, &reference.logical_path, &entry_selector).map_err(
        |primary_error| {
            format!(
                "engine.assets.definitions: Definition Entry not found ref='{}' err='{}'",
                reference.canonical, primary_error
            )
        },
    )
}

fn flatten_refs(refs: &DefinitionRefsV1) -> Vec<String> {
    let mut all = Vec::new();
    all.extend(refs.drawable_refs.iter().cloned());
    all.extend(refs.material_refs.iter().cloned());
    all.extend(refs.texture_refs.iter().cloned());
    all.extend(refs.uv_layout_refs.iter().cloned());
    all.extend(refs.physics_refs.iter().cloned());
    all.extend(refs.collision_refs.iter().cloned());
    all.extend(refs.ai_refs.iter().cloned());
    all.extend(refs.streaming_refs.iter().cloned());
    all.extend(refs.editor_refs.iter().cloned());
    all.extend(refs.other_refs.iter().cloned());
    all.sort();
    all.dedup();
    all
}

fn validate_entry(
    state: &DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> DefinitionValidationV1 {
    match load_entry(state, request) {
        Ok(entry) => DefinitionValidationV1 {
            ok: true,
            gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            definition_ref: entry.identity.definition_ref,
            code: "definitions.ok",
            message: ".ytyp Definition Entry is valid metadata; no imperative side-effect fields detected".to_owned(),
            warnings: entry.warnings,
        },
        Err(message) => DefinitionValidationV1 {
            ok: false,
            gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            definition_ref: String::new(),
            code: "definitions.invalid_entry",
            message,
            warnings: Vec::new(),
        },
    }
}

fn resolve_definition_refs(
    state: &mut DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<DefinitionRefResolutionV1, String> {
    let entry = load_entry(state, request)?;
    let flattened_refs = flatten_refs(&entry.refs);
    Ok(DefinitionRefResolutionV1 {
        ok: true,
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        definition_ref: entry.identity.definition_ref,
        refs: entry.refs,
        flattened_refs,
        resolver: ENGINE_ASSETS_GRAPH_SERVICE_ID,
        warnings: entry.warnings,
    })
}

fn describe_definition_side_effects(
    state: &mut DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<serde_json::Value, String> {
    let entry = load_entry(state, request)?;
    Ok(serde_json::json!({
        "ok": true,
        "gateway": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        "byte_owner": ENGINE_ASSET_SERVICE_ID,
        "semantic_owner": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        "definition_ref": entry.identity.definition_ref,
        "side_effect_policy": "declarative-only; allowed shape is {domain,effect,target}; imperative run_code/script/call/function fields are rejected",
        "side_effects": entry.side_effects,
        "domain_tags": entry.domain_tags,
    }))
}

fn manifest_blob(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.assets.definitions.service_manifest.v1",
            "gateway": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            "provider": "StarVaultDefinitionsRuntimeProvider",
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            "methods": DEFINITIONS_SERVICE_METHODS,
            "entry_schema": "newengine.assets.definitions.entry.v1",
            "ownership_policy": ".ytyp is metadata owned by engine.assets.definitions; not scene and not model"
        }));
    }
    let request = match manifest_request_from_payload(
        payload.as_slice(),
        definitions_method::MANIFEST_JSON_V1,
    ) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let source = match manifest_source_from_request(&request) {
        Ok(source) => source,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    match load_manifest(state, &source) {
        Ok(value) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn entry_blob(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let request =
        match ref_request_from_payload(payload.as_slice(), definitions_method::ENTRY_JSON_V1) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
    match load_entry(state, request) {
        Ok(value) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn invoke_json(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(definitions_method::VALIDATE_V1);
    match method {
        definitions_method::VALIDATE_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            ok_json(validate_entry(state, request))
        }
        definitions_method::ENTRY_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match load_entry(state, request) {
                Ok(entry) => ok_json(entry),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.entry_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::RESOLVE_REFS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match resolve_definition_refs(state, request) {
                Ok(resolution) => ok_json(resolution),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.refs_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::DESCRIBE_SIDE_EFFECTS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match describe_definition_side_effects(state, request) {
                Ok(description) => ok_json(description),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.side_effects_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::MANIFEST_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionManifestRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match manifest_source_from_request(&request)
                .and_then(|source| load_manifest(state, &source))
            {
                Ok(manifest) => ok_json(manifest),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.manifest_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.definitions: unknown invoke method '{other}'"
        ))),
    }
}

pub fn definitions_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        DEFINITIONS_SERVICE_ID,
        DEFINITIONS_GATEWAY_OWNER,
        DEFINITIONS_BACKEND_CAPABILITY_ID,
        DEFINITIONS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_DEFINITIONS_SERVICE_ID)
    .protocol(DEFINITIONS_RUNTIME_CONTRACT)
    .features(["definition-entry-v1", "metadata-namespace-preservation", "declarative-side-effects", "strict-ytyp-ownership"])
    .notes("Engine definitions runtime service. .ytyp semantics live in engine.assets.definitions; engine.assets exposes only VFS bytes and the generic NEF8 ListFile envelope.");

    JsonServiceRouter::with_state(DEFINITIONS_SERVICE_ID, DefinitionsRuntimeState::new(client))
        .describe_json(&description)
        .info(definitions_service_info)
        .blob(definitions_method::MANIFEST_JSON_V1, manifest_blob)
        .blob(definitions_method::ENTRY_JSON_V1, entry_blob)
        .post_json_result::<DefinitionRefRequest, DefinitionValidationV1, _>(
            definitions_method::VALIDATE_V1,
            |state, request| Ok(validate_entry(state, request)),
        )
        .post_json_result::<DefinitionRefRequest, DefinitionRefResolutionV1, _>(
            definitions_method::RESOLVE_REFS_V1,
            resolve_definition_refs,
        )
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(
            definitions_method::DESCRIBE_SIDE_EFFECTS_V1,
            describe_definition_side_effects,
        )
        .blob(definitions_method::INVOKE_JSON, invoke_json)
        .blob(definitions_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_definitions_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        service_kind: EngineServiceKind::Definitions,
        provider_service: DEFINITIONS_SERVICE_ID,
        provider_route: "engine.assets.starvault.definitions",
        capability: DEFINITIONS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: DEFINITIONS_GATEWAY_OWNER,
        service: definitions_gateway_service(client),
    })
}

#[cfg(test)]
mod tests;
