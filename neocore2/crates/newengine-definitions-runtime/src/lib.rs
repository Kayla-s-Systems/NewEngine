#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned `engine.definitions` runtime service.
//!
//! `.ytyp` ownership lives here. The service uses `engine.assets` only as the
//! VFS/raw-bytes/codec owner and returns Definition Entry DTOs to tools,
//! scene/map placement loaders and the asset graph resolver.

use std::collections::{BTreeMap, BTreeSet};

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_assets_api::{
    definitions_method, stable_hash_from_text, AssetDependencyRecordV1, AssetReference,
    ASSET_LIST_FILE_BODY_OUTPUT, DEFINITIONS_BACKEND_CAPABILITY_ID, DEFINITIONS_RUNTIME_CONTRACT,
    DEFINITIONS_SERVICE_ID, DEFINITIONS_SERVICE_METHODS, ENGINE_ASSET_GRAPH_SERVICE_ID,
    ENGINE_ASSET_SERVICE_ID, ENGINE_DEFINITIONS_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

pub const DEFINITIONS_GATEWAY_OWNER: &str = "newengine-definitions-runtime.engine-owned-provider";

#[derive(Clone)]
pub struct DefinitionsRuntimeState {
    client: AssetServiceClient,
}

impl DefinitionsRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self { Self { client } }
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
    fn default() -> Self { Self { definition_ref: String::new(), source: String::new(), entry: None } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionManifestRequest {
    pub source: String,
    pub definition_ref: String,
}

impl Default for DefinitionManifestRequest {
    #[inline]
    fn default() -> Self { Self { source: String::new(), definition_ref: String::new() } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct DefinitionDictionaryBodyV1 {
    schema: String,
    entries: Vec<RawDefinitionEntryV1>,
}

impl Default for DefinitionDictionaryBodyV1 {
    fn default() -> Self { Self { schema: String::new(), entries: Vec::new() } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct RawDefinitionEntryV1 {
    name: String,
    stable_hash: u64,
    entry_kind: String,
    kind: String,
    schema: String,
    target: Option<serde_json::Value>,
    dependencies: Vec<AssetDependencyRecordV1>,
    namespaces: BTreeMap<String, serde_json::Value>,
    metadata: BTreeMap<String, serde_json::Value>,
    semantic_tags: Vec<String>,
    domain_tags: Vec<String>,
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
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            side_effects: Vec::new(),
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct LegacyDefinitionManifest {
    source: String,
    definition_entries: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionIdentityV1 {
    pub name: String,
    pub source: String,
    pub definition_ref: String,
}

impl Default for DefinitionIdentityV1 {
    fn default() -> Self { Self { name: String::new(), source: String::new(), definition_ref: String::new() } }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRefsV1 {
    pub drawable_refs: Vec<String>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub collision_refs: Vec<String>,
    pub ai_refs: Vec<String>,
    pub streaming_refs: Vec<String>,
    pub editor_refs: Vec<String>,
    pub other_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionSideEffectV1 {
    pub domain: String,
    pub effect: String,
    pub target: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl Default for DefinitionSideEffectV1 {
    fn default() -> Self {
        Self { domain: String::new(), effect: String::new(), target: String::new(), metadata: BTreeMap::new() }
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
    pub side_effects: Vec<DefinitionSideEffectV1>,
    pub arbitrary_metadata: BTreeMap<String, serde_json::Value>,
    pub warnings: Vec<String>,
}

impl Default for DefinitionEntryV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.definitions.entry.v1".to_owned(),
            identity: DefinitionIdentityV1::default(),
            kind: "archetype_definition".to_owned(),
            stable_hash: 0,
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            refs: DefinitionRefsV1::default(),
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
        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
        provider: "EngineOwnedDefinitionsRuntimeProvider",
        contract: DEFINITIONS_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
        methods: DEFINITIONS_SERVICE_METHODS,
        ownership_policy: ".ytyp Definition Entry metadata is owned by engine.definitions; scene/model may consume refs but never decode or own .ytyp",
    }
}

fn normalize_logical_ref(value: &str) -> String {
    let mut s = value.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") { s = rest.to_owned(); }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") { s = s.replace("//", "/"); }
    s
}

fn definition_ref_from_request(request: &DefinitionRefRequest) -> Result<String, String> {
    if !request.definition_ref.trim().is_empty() {
        return Ok(normalize_logical_ref(&request.definition_ref));
    }
    let source = normalize_logical_ref(&request.source);
    if source.is_empty() {
        return Err("definitions.entry_json_v1 requires definition_ref='.ytyp@entry' or source + entry".to_owned());
    }
    let Some(entry) = request.entry.as_deref().map(str::trim).filter(|it| !it.is_empty()) else {
        return Err("definitions.entry_json_v1 requires .ytyp@entry; .ytyp without @entry is a dictionary manifest request".to_owned());
    };
    Ok(format!("{source}@{entry}"))
}

fn parse_definition_ref(request: &DefinitionRefRequest) -> Result<AssetReference, String> {
    let raw = definition_ref_from_request(request)?;
    newengine_assets_api::require_asset_reference_extension(&raw, &["ytyp"], true)
        .map_err(|e| e.to_string())
}

fn manifest_source_from_request(request: &DefinitionManifestRequest) -> Result<String, String> {
    let raw = if !request.source.trim().is_empty() {
        request.source.trim()
    } else if !request.definition_ref.trim().is_empty() {
        request.definition_ref.split('@').next().unwrap_or(request.definition_ref.trim())
    } else {
        return Err("definitions.manifest_json_v1 requires source='world/foo.ytyp' or definition_ref='world/foo.ytyp@entry'".to_owned());
    };
    let normalized = normalize_logical_ref(raw);
    let reference = newengine_assets_api::require_asset_reference_extension(&normalized, &["ytyp"], false)
        .map_err(|e| e.to_string())?;
    Ok(reference.logical_path)
}

fn ref_request_from_payload(payload: &[u8], method: &str) -> Result<DefinitionRefRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires definition_ref='.ytyp@entry'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionRefRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionRefRequest { definition_ref: trimmed.trim_matches('"').to_owned(), ..Default::default() })
    }
}

fn manifest_request_from_payload(payload: &[u8], method: &str) -> Result<DefinitionManifestRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires source='world/foo.ytyp'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionManifestRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionManifestRequest { source: trimmed.trim_matches('"').to_owned(), ..Default::default() })
    }
}

fn load_dictionary_body(state: &DefinitionsRuntimeState, source: &str) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let mut warnings = Vec::new();
    match state.client.decode_v1(&AssetDecodeRequest {
        logical_path: source.to_owned(),
        output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
        selector: serde_json::Value::Null,
    }) {
        Ok(body) => match serde_json::from_slice::<DefinitionDictionaryBodyV1>(&body) {
            Ok(dictionary) => return Ok((dictionary.entries, warnings)),
            Err(error) => warnings.push(format!(
                "definitions.runtime: raw .ytyp body did not match DefinitionDictionaryBodyV1; falling back to codec manifest projection; unknown metadata may be incomplete err='{error}'"
            )),
        },
        Err(error) => warnings.push(format!(
            "definitions.runtime: raw .ytyp body unavailable; falling back to codec manifest projection err='{error}'"
        )),
    }

    let bytes = state.client.decode_v1(&AssetDecodeRequest {
        logical_path: source.to_owned(),
        output_kind: definitions_method::MANIFEST_JSON_V1.to_owned(),
        selector: serde_json::Value::Null,
    }).map_err(|e| format!("engine.definitions: manifest decode failed source='{source}' err='{e}'"))?;
    let manifest = serde_json::from_slice::<LegacyDefinitionManifest>(&bytes)
        .map_err(|e| format!("engine.definitions: projected manifest was not json source='{source}' err='{e}'"))?;
    let entries = manifest.definition_entries.into_iter().map(raw_from_legacy_entry).collect();
    Ok((entries, warnings))
}

fn raw_from_legacy_entry(value: serde_json::Value) -> RawDefinitionEntryV1 {
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let entry_kind = value
        .get("entry_kind")
        .or_else(|| value.get("asset_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("archetype_definition")
        .to_owned();
    let stable_hash = value.get("stable_hash").and_then(|v| v.as_u64()).unwrap_or_else(|| stable_hash_from_text(&name));
    let mut dependencies = Vec::new();
    if let Some(drawable) = value.pointer("/dictionaries/drawable").and_then(|v| v.as_str()) {
        dependencies.push(AssetDependencyRecordV1::new(drawable, "drawable", "engine.model", true));
    }
    if let Some(texture) = value.pointer("/dictionaries/texture").and_then(|v| v.as_str()) {
        dependencies.push(AssetDependencyRecordV1::new(texture, "texture", "engine.textures", true));
    }
    if let Some(physics) = value.pointer("/dictionaries/physics").and_then(|v| v.as_str()) {
        dependencies.push(AssetDependencyRecordV1::new(physics, "physics", "engine.physics", true));
    }
    RawDefinitionEntryV1 {
        name,
        stable_hash,
        entry_kind,
        dependencies,
        metadata: BTreeMap::from([("legacy_projected_definition".to_owned(), value)]),
        ..Default::default()
    }
}

fn entry_matches(raw: &RawDefinitionEntryV1, selector: &str) -> bool {
    if raw.name.eq_ignore_ascii_case(selector) {
        return true;
    }
    if let Some(rest) = selector.strip_prefix("hash:") {
        return rest.parse::<u64>().map(|hash| hash == effective_hash(raw)).unwrap_or(false);
    }
    false
}

fn effective_kind(raw: &RawDefinitionEntryV1) -> String {
    for candidate in [&raw.kind, &raw.entry_kind] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() { return trimmed.to_owned(); }
    }
    "archetype_definition".to_owned()
}

fn effective_hash(raw: &RawDefinitionEntryV1) -> u64 {
    if raw.stable_hash == 0 { stable_hash_from_text(&raw.name) } else { raw.stable_hash }
}

fn value_collect_tags(value: &serde_json::Value, key_hint: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) => {
            if key_hint.contains("tag") || key_hint.contains("domain") {
                let t = text.trim();
                if !t.is_empty() { out.insert(t.to_owned()); }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items { value_collect_tags(item, key_hint, out); }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map { value_collect_tags(v, k, out); }
        }
        _ => {}
    }
}

fn collect_tags(raw: &RawDefinitionEntryV1) -> (Vec<String>, Vec<String>) {
    let mut semantic = BTreeSet::new();
    let mut domain = BTreeSet::new();
    for tag in &raw.semantic_tags {
        let t = tag.trim();
        if !t.is_empty() { semantic.insert(t.to_owned()); }
    }
    for tag in &raw.domain_tags {
        let t = tag.trim();
        if !t.is_empty() { domain.insert(t.to_owned()); }
    }
    for (ns, value) in &raw.namespaces {
        if !ns.trim().is_empty() { domain.insert(ns.to_owned()); }
        value_collect_tags(value, ns, &mut semantic);
    }
    for (ns, value) in &raw.metadata {
        if ns.contains("domain") || ns.starts_with("engine.") { domain.insert(ns.to_owned()); }
        value_collect_tags(value, ns, &mut semantic);
    }
    (semantic.into_iter().collect(), domain.into_iter().collect())
}

fn classify_ref(reference: &str, role: &str, domain: &str, refs: &mut DefinitionRefsV1) {
    let reference = normalize_logical_ref(reference);
    if reference.is_empty() { return; }
    let lower = reference.to_ascii_lowercase();
    let hint = format!("{} {}", role.to_ascii_lowercase(), domain.to_ascii_lowercase());
    let bucket = if lower.contains(".ydd@") || hint.contains("drawable") || hint.contains("model") {
        &mut refs.drawable_refs
    } else if lower.contains(".nemat@") || hint.contains("material") {
        &mut refs.material_refs
    } else if lower.contains(".ytd@") || hint.contains("texture") {
        &mut refs.texture_refs
    } else if lower.contains(".ybn@") || lower.contains(".ycol@") || hint.contains("collision") {
        &mut refs.collision_refs
    } else if hint.contains("physics") {
        &mut refs.physics_refs
    } else if lower.contains(".nebrain@") || lower.contains(".nepat@") || lower.contains(".nemem@") || hint.contains("ai") {
        &mut refs.ai_refs
    } else if hint.contains("stream") {
        &mut refs.streaming_refs
    } else if hint.contains("editor") {
        &mut refs.editor_refs
    } else {
        &mut refs.other_refs
    };
    if !bucket.iter().any(|it| it == &reference) {
        bucket.push(reference);
    }
}

fn collect_refs_from_value(value: &serde_json::Value, key_hint: &str, refs: &mut DefinitionRefsV1) {
    match value {
        serde_json::Value::String(text) => {
            let normalized = normalize_logical_ref(text);
            let lower = normalized.to_ascii_lowercase();
            if [".ydd@", ".nemat@", ".ytd@", ".ybn@", ".ycol@", ".nebrain@", ".nepat@", ".nemem@", ".ytyp@"].iter().any(|needle| lower.contains(needle)) {
                classify_ref(&normalized, key_hint, key_hint, refs);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items { collect_refs_from_value(item, key_hint, refs); }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map { collect_refs_from_value(v, k, refs); }
        }
        _ => {}
    }
}

fn collect_refs(raw: &RawDefinitionEntryV1) -> DefinitionRefsV1 {
    let mut refs = DefinitionRefsV1::default();
    for dep in &raw.dependencies {
        classify_ref(&dep.reference, &dep.role, &dep.domain, &mut refs);
    }
    if let Some(target) = &raw.target {
        collect_refs_from_value(target, "target", &mut refs);
    }
    for (key, value) in &raw.namespaces {
        collect_refs_from_value(value, key, &mut refs);
    }
    for (key, value) in &raw.metadata {
        collect_refs_from_value(value, key, &mut refs);
    }
    refs
}

fn imperative_field_name(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "run_code" | "script" | "script_body" | "eval" | "function" | "call" | "callback" | "command" | "imperative" | "spawn_logic"
    )
}

fn side_effect_from_value(value: &serde_json::Value, out: &mut Vec<DefinitionSideEffectV1>, errors: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items { side_effect_from_value(item, out, errors); }
        }
        serde_json::Value::Object(map) => {
            for key in map.keys() {
                if imperative_field_name(key) {
                    errors.push(format!("imperative side-effect field '{key}' is forbidden; use descriptive domain/effect/target metadata only"));
                }
            }
            let domain = map.get("domain").and_then(|v| v.as_str()).unwrap_or_default().trim();
            let effect = map.get("effect").and_then(|v| v.as_str()).unwrap_or_default().trim();
            let target = map.get("target").and_then(|v| v.as_str()).unwrap_or_default().trim();
            if !domain.is_empty() || !effect.is_empty() || !target.is_empty() {
                if domain.is_empty() || effect.is_empty() || target.is_empty() {
                    errors.push("side-effect declaration requires domain, effect and target".to_owned());
                } else {
                    let metadata = map
                        .iter()
                        .filter(|(k, _)| *k != "domain" && *k != "effect" && *k != "target")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    out.push(DefinitionSideEffectV1 { domain: domain.to_owned(), effect: effect.to_owned(), target: target.to_owned(), metadata });
                }
            }
            for (k, v) in map {
                if k == "domain" || k == "effect" || k == "target" { continue; }
                side_effect_from_value(v, out, errors);
            }
        }
        _ => {}
    }
}

fn collect_side_effects(raw: &RawDefinitionEntryV1) -> Result<Vec<DefinitionSideEffectV1>, Vec<String>> {
    let mut side_effects = raw.side_effects.clone();
    let mut errors = Vec::new();
    for effect in &side_effects {
        if effect.domain.trim().is_empty() || effect.effect.trim().is_empty() || effect.target.trim().is_empty() {
            errors.push("side-effect declaration requires non-empty domain, effect and target".to_owned());
        }
        for key in effect.metadata.keys() {
            if imperative_field_name(key) {
                errors.push(format!("imperative side-effect field '{key}' is forbidden"));
            }
        }
    }
    for key in ["side_effects", "sideEffects", "effects", "runtime_side_effects"] {
        if let Some(value) = raw.metadata.get(key).or_else(|| raw.namespaces.get(key)) {
            side_effect_from_value(value, &mut side_effects, &mut errors);
        }
    }
    if let Some(target) = &raw.target {
        side_effect_from_value(target, &mut side_effects, &mut errors);
    }
    if errors.is_empty() { Ok(side_effects) } else { Err(errors) }
}

fn arbitrary_metadata(raw: &RawDefinitionEntryV1) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("namespaces".to_owned(), serde_json::to_value(&raw.namespaces).unwrap_or_default());
    metadata.insert("metadata".to_owned(), serde_json::to_value(&raw.metadata).unwrap_or_default());
    metadata.insert("target".to_owned(), raw.target.clone().unwrap_or(serde_json::Value::Null));
    metadata.insert("dependencies".to_owned(), serde_json::to_value(&raw.dependencies).unwrap_or_default());
    metadata.insert("flags".to_owned(), serde_json::json!(raw.flags));
    let mut unknown = BTreeSet::new();
    for key in raw.namespaces.keys().chain(raw.metadata.keys()) {
        if !key.starts_with("newengine.") && !key.starts_with("engine.") {
            unknown.insert(key.clone());
        }
    }
    metadata.insert("unknown_metadata_namespaces".to_owned(), serde_json::json!(unknown.into_iter().collect::<Vec<_>>()));
    metadata
}

fn build_entry(source: &str, raw: RawDefinitionEntryV1, inherited_warnings: &[String]) -> Result<DefinitionEntryV1, String> {
    let name = raw.name.trim().to_owned();
    if name.is_empty() {
        return Err(".ytyp Definition Entry has empty identity.name".to_owned());
    }
    let side_effects = collect_side_effects(&raw).map_err(|errors| errors.join("; "))?;
    let stable_hash = effective_hash(&raw);
    let (semantic_tags, domain_tags) = collect_tags(&raw);
    Ok(DefinitionEntryV1 {
        identity: DefinitionIdentityV1 {
            name: name.clone(),
            source: source.to_owned(),
            definition_ref: format!("{source}@{name}"),
        },
        kind: effective_kind(&raw),
        stable_hash,
        semantic_tags,
        domain_tags,
        refs: collect_refs(&raw),
        side_effects,
        arbitrary_metadata: arbitrary_metadata(&raw),
        warnings: inherited_warnings.to_vec(),
        ..Default::default()
    })
}

fn load_manifest(state: &DefinitionsRuntimeState, source: &str) -> Result<DefinitionManifestV1, String> {
    let (raw_entries, warnings) = load_dictionary_body(state, source)?;
    let mut entries = Vec::with_capacity(raw_entries.len());
    for raw in raw_entries {
        let name = raw.name.trim().to_owned();
        if name.is_empty() { continue; }
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
    entries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.stable_hash.cmp(&b.stable_hash)));
    Ok(DefinitionManifestV1 {
        schema: "newengine.definitions.manifest.v1",
        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
        source: source.to_owned(),
        status: "definition_dictionary_manifest",
        entries,
        warnings,
    })
}

fn load_entry(state: &DefinitionsRuntimeState, request: DefinitionRefRequest) -> Result<DefinitionEntryV1, String> {
    let reference = parse_definition_ref(&request)?;
    let entry_selector = reference.entry.as_deref().unwrap_or_default().to_owned();
    let (raw_entries, warnings) = load_dictionary_body(state, &reference.logical_path)?;
    for raw in raw_entries {
        if entry_matches(&raw, &entry_selector) {
            return build_entry(&reference.logical_path, raw, &warnings);
        }
    }
    Err(format!("engine.definitions: Definition Entry not found ref='{}'", reference.canonical))
}

fn flatten_refs(refs: &DefinitionRefsV1) -> Vec<String> {
    let mut all = Vec::new();
    all.extend(refs.drawable_refs.iter().cloned());
    all.extend(refs.material_refs.iter().cloned());
    all.extend(refs.texture_refs.iter().cloned());
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

fn validate_entry(state: &DefinitionsRuntimeState, request: DefinitionRefRequest) -> DefinitionValidationV1 {
    match load_entry(state, request) {
        Ok(entry) => DefinitionValidationV1 {
            ok: true,
            gateway: ENGINE_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
            definition_ref: entry.identity.definition_ref,
            code: "definitions.ok",
            message: ".ytyp Definition Entry is valid metadata; no imperative side-effect fields detected".to_owned(),
            warnings: entry.warnings,
        },
        Err(message) => DefinitionValidationV1 {
            ok: false,
            gateway: ENGINE_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
            definition_ref: String::new(),
            code: "definitions.invalid_entry",
            message,
            warnings: Vec::new(),
        },
    }
}

fn manifest_blob(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.definitions.service_manifest.v1",
            "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
            "provider": "EngineOwnedDefinitionsRuntimeProvider",
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_DEFINITIONS_SERVICE_ID,
            "methods": DEFINITIONS_SERVICE_METHODS,
            "entry_schema": "newengine.definitions.entry.v1",
            "ownership_policy": ".ytyp is metadata owned by engine.definitions; not scene and not model"
        }));
    }
    let request = match manifest_request_from_payload(payload.as_slice(), definitions_method::MANIFEST_JSON_V1) {
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
    let request = match ref_request_from_payload(payload.as_slice(), definitions_method::ENTRY_JSON_V1) {
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
    let method = value.get("method").and_then(|v| v.as_str()).unwrap_or(definitions_method::VALIDATE_V1);
    match method {
        definitions_method::VALIDATE_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(value.get("request").cloned().unwrap_or_default()).unwrap_or_default();
            ok_json(validate_entry(state, request))
        }
        definitions_method::ENTRY_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(value.get("request").cloned().unwrap_or_default()).unwrap_or_default();
            match load_entry(state, request) {
                Ok(entry) => ok_json(entry),
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.entry_unavailable", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID }),
            }
        }
        definitions_method::RESOLVE_REFS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(value.get("request").cloned().unwrap_or_default()).unwrap_or_default();
            match load_entry(state, request) {
                Ok(entry) => {
                    let flattened_refs = flatten_refs(&entry.refs);
                    ok_json(DefinitionRefResolutionV1 {
                        ok: true,
                        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
                        byte_owner: ENGINE_ASSET_SERVICE_ID,
                        semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
                        definition_ref: entry.identity.definition_ref,
                        refs: entry.refs,
                        flattened_refs,
                        resolver: ENGINE_ASSET_GRAPH_SERVICE_ID,
                        warnings: entry.warnings,
                    })
                }
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.refs_unavailable", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID }),
            }
        }
        definitions_method::DESCRIBE_SIDE_EFFECTS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(value.get("request").cloned().unwrap_or_default()).unwrap_or_default();
            match load_entry(state, request) {
                Ok(entry) => ok_json(serde_json::json!({
                    "ok": true,
                    "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                    "byte_owner": ENGINE_ASSET_SERVICE_ID,
                    "semantic_owner": ENGINE_DEFINITIONS_SERVICE_ID,
                    "definition_ref": entry.identity.definition_ref,
                    "side_effect_policy": "declarative-only; allowed shape is {domain,effect,target}; imperative run_code/script/call/function fields are rejected",
                    "side_effects": entry.side_effects,
                    "domain_tags": entry.domain_tags,
                })),
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.side_effects_unavailable", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID }),
            }
        }
        definitions_method::MANIFEST_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionManifestRequest>(value.get("request").cloned().unwrap_or_default()).unwrap_or_default();
            match manifest_source_from_request(&request).and_then(|source| load_manifest(state, &source)) {
                Ok(manifest) => ok_json(manifest),
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.manifest_unavailable", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID }),
            }
        }
        other => RResult::RErr(RString::from(format!("engine.definitions: unknown invoke method '{other}'"))),
    }
}

pub fn definitions_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        DEFINITIONS_SERVICE_ID,
        DEFINITIONS_GATEWAY_OWNER,
        DEFINITIONS_BACKEND_CAPABILITY_ID,
        DEFINITIONS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_DEFINITIONS_SERVICE_ID)
    .protocol(DEFINITIONS_RUNTIME_CONTRACT)
    .features(["definition-entry-v1", "metadata-namespace-preservation", "declarative-side-effects", "strict-ytyp-ownership"])
    .notes("Engine definitions runtime service. .ytyp semantics live in engine.definitions; VFS/raw bytes/codec dispatch remain in engine.assets.");

    JsonServiceRouter::with_state(DEFINITIONS_SERVICE_ID, DefinitionsRuntimeState::new(client))
        .describe_json(&description)
        .info(definitions_service_info)
        .blob(definitions_method::MANIFEST_JSON_V1, manifest_blob)
        .blob(definitions_method::ENTRY_JSON_V1, entry_blob)
        .post_json_result::<DefinitionRefRequest, DefinitionValidationV1, _>(definitions_method::VALIDATE_V1, |state, request| Ok(validate_entry(state, request)))
        .post_json_result::<DefinitionRefRequest, DefinitionRefResolutionV1, _>(definitions_method::RESOLVE_REFS_V1, |state, request| {
            let entry = load_entry(state, request)?;
            let flattened_refs = flatten_refs(&entry.refs);
            Ok(DefinitionRefResolutionV1 {
                ok: true,
                gateway: ENGINE_DEFINITIONS_SERVICE_ID,
                byte_owner: ENGINE_ASSET_SERVICE_ID,
                semantic_owner: ENGINE_DEFINITIONS_SERVICE_ID,
                definition_ref: entry.identity.definition_ref,
                refs: entry.refs,
                flattened_refs,
                resolver: ENGINE_ASSET_GRAPH_SERVICE_ID,
                warnings: entry.warnings,
            })
        })
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::DESCRIBE_SIDE_EFFECTS_V1, |state, request| {
            let entry = load_entry(state, request)?;
            Ok(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "byte_owner": ENGINE_ASSET_SERVICE_ID,
                "semantic_owner": ENGINE_DEFINITIONS_SERVICE_ID,
                "definition_ref": entry.identity.definition_ref,
                "side_effect_policy": "declarative-only; allowed shape is {domain,effect,target}; imperative run_code/script/call/function fields are rejected",
                "side_effects": entry.side_effects,
                "domain_tags": entry.domain_tags,
            }))
        })
        .blob(definitions_method::INVOKE_JSON, invoke_json)
        .blob(definitions_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_definitions_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
        service_kind: EngineServiceKind::Definitions,
        provider_service: DEFINITIONS_SERVICE_ID,
        capability: DEFINITIONS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: DEFINITIONS_GATEWAY_OWNER,
        service: definitions_gateway_service(client),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imperative_side_effect_fields_are_rejected() {
        let raw = RawDefinitionEntryV1 {
            name: "garage".to_owned(),
            metadata: BTreeMap::from([(
                "side_effects".to_owned(),
                serde_json::json!([{ "run_code": "spawnGarageHardcodedLogic()" }]),
            )]),
            ..Default::default()
        };
        assert!(collect_side_effects(&raw).is_err());
    }

    #[test]
    fn declarative_side_effect_is_allowed() {
        let raw = RawDefinitionEntryV1 {
            name: "body".to_owned(),
            metadata: BTreeMap::from([(
                "side_effects".to_owned(),
                serde_json::json!([{ "domain": "engine.model", "effect": "require_drawable", "target": "models/foo.ydd@body" }]),
            )]),
            ..Default::default()
        };
        let effects = collect_side_effects(&raw).unwrap();
        assert_eq!(effects[0].domain, "engine.model");
    }

    #[test]
    fn refs_are_classified_by_extension() {
        let raw = RawDefinitionEntryV1 {
            name: "body".to_owned(),
            dependencies: vec![
                AssetDependencyRecordV1::new("models/foo.ydd@body", "drawable", "engine.model", true),
                AssetDependencyRecordV1::new("materials/foo.nemat@body", "material", "engine.materials", true),
                AssetDependencyRecordV1::new("textures/foo.ytd@diff", "texture", "engine.textures", true),
            ],
            ..Default::default()
        };
        let refs = collect_refs(&raw);
        assert_eq!(refs.drawable_refs, vec!["models/foo.ydd@body"]);
        assert_eq!(refs.material_refs, vec!["materials/foo.nemat@body"]);
        assert_eq!(refs.texture_refs, vec!["textures/foo.ytd@diff"]);
    }
}
