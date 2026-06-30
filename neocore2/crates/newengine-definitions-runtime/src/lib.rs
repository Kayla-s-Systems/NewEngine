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
    let mut s = value.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    s
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

fn xml_attr_string(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> Option<String> {
    authored_xml::xml_attr_any(node, names)
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
}

fn xml_attr_bool(node: authored_xml::XmlNode<'_, '_>, names: &[&str], default: bool) -> bool {
    xml_attr_string(node, names)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "required"
            )
        })
        .unwrap_or(default)
}

fn xml_attr_u64(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> u64 {
    xml_attr_string(node, names)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default()
}

fn xml_attr_u32(node: authored_xml::XmlNode<'_, '_>, names: &[&str]) -> u32 {
    xml_attr_string(node, names)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_default()
}

fn xml_tags(container: Option<authored_xml::XmlNode<'_, '_>>) -> Vec<String> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Tag"))
        .filter_map(|tag| xml_attr_string(tag, &["value", "name", "tag"]))
        .collect()
}

fn xml_dependencies(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<AssetDependencyRecordV1> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Dependency"))
        .filter_map(|dep| {
            let reference = xml_attr_string(dep, &["reference", "ref", "path"])?;
            let role =
                xml_attr_string(dep, &["role", "kind"]).unwrap_or_else(|| "dependency".to_owned());
            let domain = xml_attr_string(dep, &["domain"]).unwrap_or_default();
            Some(AssetDependencyRecordV1::new(
                reference,
                role,
                domain,
                xml_attr_bool(dep, &["required"], true),
            ))
        })
        .collect()
}

fn xml_material_bindings(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<MaterialBindingRef> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Binding"))
        .filter_map(|binding| {
            let slot = xml_attr_string(binding, &["slot", "name"])?;
            let material_ref = xml_attr_string(binding, &["material_ref", "material", "ref"])?;
            Some(MaterialBindingRef {
                slot,
                material_ref,
                required: xml_attr_bool(binding, &["required"], true),
            })
        })
        .collect()
}

fn xml_side_effects(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> Vec<DefinitionSideEffectV1> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("SideEffect"))
        .map(|effect| DefinitionSideEffectV1 {
            domain: xml_attr_string(effect, &["domain"]).unwrap_or_default(),
            effect: xml_attr_string(effect, &["effect", "name"]).unwrap_or_default(),
            target: xml_attr_string(effect, &["target"]).unwrap_or_default(),
            metadata: BTreeMap::new(),
        })
        .collect()
}

fn xml_render_namespace_value(ns: authored_xml::XmlNode<'_, '_>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for child in ns.children().filter(|child| child.is_element()) {
        if child.has_tag_name("Value") {
            if let (Some(key), Some(value)) = (
                xml_attr_string(child, &["key", "name"]),
                xml_attr_string(child, &["value"]),
            ) {
                map.insert(key, serde_json::Value::String(value));
            }
        } else {
            map.insert(
                child.tag_name().name().to_owned(),
                authored_xml::xml_node_object(child),
            );
        }
    }
    serde_json::Value::Object(map)
}

fn xml_metadata_namespaces(
    container: Option<authored_xml::XmlNode<'_, '_>>,
) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    let Some(container) = container else {
        return out;
    };
    for ns in container
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Namespace"))
    {
        let Some(name) = xml_attr_string(ns, &["name", "namespace"]) else {
            continue;
        };
        let value = if name == "render" || name == "newengine.render" {
            xml_render_namespace_value(ns)
        } else {
            authored_xml::xml_node_children_object(ns)
        };
        out.insert(name, value);
    }
    out
}

fn parse_ytyp_xml_document(
    source: &str,
    body: &[u8],
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let document = authored_xml::parse_xml_body(body, source)?;
    let root = document.root_element();
    if !authored_xml::root_has_any_name(
        root,
        &["YtypProperties", "YtypDictionary", "DefinitionEntry"],
    ) {
        return Err(format!(
            "engine.assets.definitions: unsupported .ytyp XML root source='{source}' actual='{}'",
            root.tag_name().name()
        ));
    }
    let mut raw = RawDefinitionEntryV1 {
        name: xml_attr_string(root, &["name", "id", "asset_name"]).unwrap_or_else(|| {
            source
                .rsplit('/')
                .next()
                .unwrap_or(source)
                .trim_end_matches(".ytyp")
                .to_owned()
        }),
        stable_hash: xml_attr_u64(root, &["stable_hash", "stableHash"]),
        entry_kind: xml_attr_string(root, &["entry_kind", "entryKind"])
            .unwrap_or_else(|| "archetype_definition".to_owned()),
        kind: xml_attr_string(root, &["kind"]).unwrap_or_default(),
        schema: xml_attr_string(root, &["schema"])
            .unwrap_or_else(|| "newengine.ytyp.properties.v1".to_owned()),
        flags: xml_attr_u32(root, &["flags"]),
        ..Default::default()
    };
    raw.dependencies = xml_dependencies(authored_xml::xml_child(root, "Dependencies"));
    raw.material_bindings =
        xml_material_bindings(authored_xml::xml_child(root, "MaterialBindings"));
    raw.semantic_tags = xml_tags(authored_xml::xml_child(root, "SemanticTags"));
    raw.domain_tags = xml_tags(authored_xml::xml_child(root, "DomainTags"));
    raw.namespaces = authored_xml::xml_child(root, "Namespaces")
        .map(authored_xml::xml_namespace_map)
        .unwrap_or_default();
    raw.metadata = xml_metadata_namespaces(authored_xml::xml_child(root, "Metadata"));
    raw.side_effects = xml_side_effects(authored_xml::xml_child(root, "SideEffects"));
    Ok((
        vec![raw],
        vec![format!(
            ".ytyp parsed as XML authoring schema='{}' source='{}'",
            authored_xml::root_schema(root),
            source
        )],
    ))
}

fn json_string_at(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn raw_definition_entry_from_json(
    source: &str,
    value: &serde_json::Value,
) -> Result<RawDefinitionEntryV1, String> {
    let mut raw = serde_json::from_value::<RawDefinitionEntryV1>(value.clone()).map_err(|error| {
        format!("engine.assets.definitions: invalid .ytyp JSON entry source='{source}' err='{error}'")
    })?;
    if raw.name.trim().is_empty() {
        raw.name = json_string_at(value, &["identity", "name"])
            .or_else(|| json_string_at(value, &["asset", "name"]))
            .or_else(|| json_string_at(value, &["archetype", "name"]))
            .unwrap_or_default();
    }
    if raw.schema.trim().is_empty() {
        raw.schema = json_string_at(value, &["schema"])
            .unwrap_or_else(|| "newengine.ytyp.definition_entry.v1".to_owned());
    }
    Ok(raw)
}

fn parse_ytyp_json_entries(
    source: &str,
    value: &serde_json::Value,
) -> Result<Vec<RawDefinitionEntryV1>, String> {
    if let Some(entries) = value
        .get("entries")
        .or_else(|| value.get("definition_entries"))
        .or_else(|| value.get("definitionEntries"))
        .and_then(|v| v.as_array())
    {
        return entries
            .iter()
            .map(|entry| raw_definition_entry_from_json(source, entry))
            .collect();
    }
    if let Some(entry) = value.get("entry").or_else(|| value.get("definition_entry")) {
        return Ok(vec![raw_definition_entry_from_json(source, entry)?]);
    }
    if let Some(entries) = value.as_array() {
        return entries
            .iter()
            .map(|entry| raw_definition_entry_from_json(source, entry))
            .collect();
    }
    if value.is_object() {
        return Ok(vec![raw_definition_entry_from_json(source, value)?]);
    }
    Err(format!(
        "engine.assets.definitions: .ytyp JSON root must be object or array source='{source}'"
    ))
}

fn parse_ytyp_json_document(
    source: &str,
    body: &[u8],
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).map_err(|error| {
        format!(
            "engine.assets.definitions: .ytyp JSON body is invalid source='{source}' err='{error}'"
        )
    })?;
    let schema = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("newengine.ytyp.dictionary.v1");
    match schema {
        "newengine.ytyp.dictionary.v1"
        | "newengine.ytyp.archetype_dictionary.v1"
        | "newengine.ytyp.definition_entry.v1"
        | "newengine.ytyp.properties.v1" => {}
        other => {
            return Err(format!(
                "engine.assets.definitions: unsupported .ytyp JSON schema source='{source}' expected='newengine.ytyp.dictionary.v1' actual='{other}'"
            ));
        }
    }
    let entries = parse_ytyp_json_entries(source, &value)?;
    if entries.is_empty() {
        return Err(format!("source='{source}' contains no .ytyp entries"));
    }
    Ok((
        entries,
        vec![format!(
            ".ytyp parsed as JSON schema='{schema}' entries_source='{}'",
            source
        )],
    ))
}

fn entry_matches(raw: &RawDefinitionEntryV1, selector: &str) -> bool {
    if raw.name.eq_ignore_ascii_case(selector) {
        return true;
    }
    if let Some(rest) = selector.strip_prefix("hash:") {
        return rest
            .parse::<u64>()
            .map(|hash| hash == effective_hash(raw))
            .unwrap_or(false);
    }
    false
}

fn effective_kind(raw: &RawDefinitionEntryV1) -> String {
    for candidate in [&raw.kind, &raw.entry_kind] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    "archetype_definition".to_owned()
}

fn effective_hash(raw: &RawDefinitionEntryV1) -> u64 {
    if raw.stable_hash == 0 {
        stable_hash_from_text(&raw.name)
    } else {
        raw.stable_hash
    }
}

fn value_collect_tags(value: &serde_json::Value, key_hint: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) => {
            if key_hint.contains("tag") || key_hint.contains("domain") {
                let t = text.trim();
                if !t.is_empty() {
                    out.insert(t.to_owned());
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                value_collect_tags(item, key_hint, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                value_collect_tags(v, k, out);
            }
        }
        _ => {}
    }
}

fn collect_tags(raw: &RawDefinitionEntryV1) -> (Vec<String>, Vec<String>) {
    let mut semantic = BTreeSet::new();
    let mut domain = BTreeSet::new();
    for tag in &raw.semantic_tags {
        let t = tag.trim();
        if !t.is_empty() {
            semantic.insert(t.to_owned());
        }
    }
    for tag in &raw.domain_tags {
        let t = tag.trim();
        if !t.is_empty() {
            domain.insert(t.to_owned());
        }
    }
    for (ns, value) in &raw.namespaces {
        if !ns.trim().is_empty() {
            domain.insert(ns.to_owned());
        }
        value_collect_tags(value, ns, &mut semantic);
    }
    for (ns, value) in &raw.metadata {
        if ns.contains("domain") || ns.starts_with("engine.") {
            domain.insert(ns.to_owned());
        }
        value_collect_tags(value, ns, &mut semantic);
    }
    (semantic.into_iter().collect(), domain.into_iter().collect())
}

fn classify_ref(reference: &str, role: &str, domain: &str, refs: &mut DefinitionRefsV1) {
    let reference = normalize_logical_ref(reference);
    if reference.is_empty() {
        return;
    }
    let lower = reference.to_ascii_lowercase();
    let hint = format!(
        "{} {}",
        role.to_ascii_lowercase(),
        domain.to_ascii_lowercase()
    );
    let bucket = if lower.contains(".ytyd@") || hint.contains("uv") || hint.contains("unwrap") {
        &mut refs.uv_layout_refs
    } else if lower.contains(".ydd@") || hint.contains("drawable") || hint.contains("model") {
        &mut refs.drawable_refs
    } else if lower.contains(".nemat@") || hint.contains("material") {
        &mut refs.material_refs
    } else if lower.contains(".ytd@") || hint.contains("texture") {
        &mut refs.texture_refs
    } else if lower.contains(".ybn@") || lower.contains(".ycol@") || hint.contains("collision") {
        &mut refs.collision_refs
    } else if hint.contains("physics") {
        &mut refs.physics_refs
    } else if lower.contains(".nebrain@")
        || lower.contains(".nepat@")
        || lower.contains(".nemem@")
        || hint.contains("ai")
    {
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
            if [
                ".ydd@",
                ".nemat@",
                ".ytd@",
                ".ytyd@",
                ".ybn@",
                ".ycol@",
                ".nebrain@",
                ".nepat@",
                ".nemem@",
                ".ytyp@",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
            {
                classify_ref(&normalized, key_hint, key_hint, refs);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_refs_from_value(item, key_hint, refs);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                collect_refs_from_value(v, k, refs);
            }
        }
        _ => {}
    }
}

fn collect_refs(raw: &RawDefinitionEntryV1) -> DefinitionRefsV1 {
    let mut refs = DefinitionRefsV1::default();
    for dep in &raw.dependencies {
        classify_ref(&dep.reference, &dep.role, &dep.domain, &mut refs);
    }
    for binding in &raw.material_bindings {
        classify_ref(
            &binding.material_ref,
            &format!("material_slot/{}", binding.slot),
            "engine.assets.materials",
            &mut refs,
        );
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
        "run_code"
            | "script"
            | "script_body"
            | "eval"
            | "function"
            | "call"
            | "callback"
            | "command"
            | "imperative"
            | "spawn_logic"
    )
}

fn side_effect_from_value(
    value: &serde_json::Value,
    out: &mut Vec<DefinitionSideEffectV1>,
    errors: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                side_effect_from_value(item, out, errors);
            }
        }
        serde_json::Value::Object(map) => {
            for key in map.keys() {
                if imperative_field_name(key) {
                    errors.push(format!("imperative side-effect field '{key}' is forbidden; use descriptive domain/effect/target metadata only"));
                }
            }
            let domain = map
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            let effect = map
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            let target = map
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            if !domain.is_empty() || !effect.is_empty() || !target.is_empty() {
                if domain.is_empty() || effect.is_empty() || target.is_empty() {
                    errors.push(
                        "side-effect declaration requires domain, effect and target".to_owned(),
                    );
                } else {
                    let metadata = map
                        .iter()
                        .filter(|(k, _)| *k != "domain" && *k != "effect" && *k != "target")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    out.push(DefinitionSideEffectV1 {
                        domain: domain.to_owned(),
                        effect: effect.to_owned(),
                        target: target.to_owned(),
                        metadata,
                    });
                }
            }
            for (k, v) in map {
                if k == "domain" || k == "effect" || k == "target" {
                    continue;
                }
                side_effect_from_value(v, out, errors);
            }
        }
        _ => {}
    }
}

fn collect_side_effects(
    raw: &RawDefinitionEntryV1,
) -> Result<Vec<DefinitionSideEffectV1>, Vec<String>> {
    let mut side_effects = raw.side_effects.clone();
    let mut errors = Vec::new();
    for effect in &side_effects {
        if effect.domain.trim().is_empty()
            || effect.effect.trim().is_empty()
            || effect.target.trim().is_empty()
        {
            errors.push(
                "side-effect declaration requires non-empty domain, effect and target".to_owned(),
            );
        }
        for key in effect.metadata.keys() {
            if imperative_field_name(key) {
                errors.push(format!("imperative side-effect field '{key}' is forbidden"));
            }
        }
    }
    for key in [
        "side_effects",
        "sideEffects",
        "effects",
        "runtime_side_effects",
    ] {
        if let Some(value) = raw.metadata.get(key).or_else(|| raw.namespaces.get(key)) {
            side_effect_from_value(value, &mut side_effects, &mut errors);
        }
    }
    if let Some(target) = &raw.target {
        side_effect_from_value(target, &mut side_effects, &mut errors);
    }
    if errors.is_empty() {
        Ok(side_effects)
    } else {
        Err(errors)
    }
}

fn arbitrary_metadata(raw: &RawDefinitionEntryV1) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "namespaces".to_owned(),
        serde_json::to_value(&raw.namespaces).unwrap_or_default(),
    );
    metadata.insert(
        "metadata".to_owned(),
        serde_json::to_value(&raw.metadata).unwrap_or_default(),
    );
    metadata.insert(
        "target".to_owned(),
        raw.target.clone().unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "dependencies".to_owned(),
        serde_json::to_value(&raw.dependencies).unwrap_or_default(),
    );
    metadata.insert(
        "material_bindings".to_owned(),
        serde_json::to_value(&raw.material_bindings).unwrap_or_default(),
    );
    metadata.insert("flags".to_owned(), serde_json::json!(raw.flags));
    let mut unknown = BTreeSet::new();
    for key in raw.namespaces.keys().chain(raw.metadata.keys()) {
        if !key.starts_with("newengine.") && !key.starts_with("engine.") {
            unknown.insert(key.clone());
        }
    }
    metadata.insert(
        "unknown_metadata_namespaces".to_owned(),
        serde_json::json!(unknown.into_iter().collect::<Vec<_>>()),
    );
    metadata
}

fn raw_has_tag(raw: &RawDefinitionEntryV1, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    raw.semantic_tags
        .iter()
        .chain(raw.domain_tags.iter())
        .any(|tag| tag.to_ascii_lowercase() == needle)
}

fn value_string_for_key(value: &serde_json::Value, wanted_key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if map
                .get("key")
                .and_then(|v| v.as_str())
                .map(|key| key.eq_ignore_ascii_case(wanted_key))
                .unwrap_or(false)
            {
                if let Some(text) = map.get("value").and_then(|v| v.as_str()) {
                    return Some(text.to_owned());
                }
            }
            if let Some(text) = map.get(wanted_key).and_then(|v| v.as_str()) {
                return Some(text.to_owned());
            }
            for value in map.values() {
                if let Some(found) = value_string_for_key(value, wanted_key) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| value_string_for_key(item, wanted_key)),
        _ => None,
    }
}

fn raw_render_string(raw: &RawDefinitionEntryV1, key: &str) -> Option<String> {
    raw.metadata
        .get("render")
        .or_else(|| raw.metadata.get("newengine.render"))
        .or_else(|| raw.namespaces.get("render"))
        .or_else(|| raw.namespaces.get("newengine.render"))
        .and_then(|value| value_string_for_key(value, key))
}

fn render_options_from_role(role: &str) -> Option<MeshRenderOptions> {
    match role.trim().to_ascii_lowercase().as_str() {
        "world_opaque" | "opaque" => Some(MeshRenderOptions::world_opaque()),
        "terrain_patch" | "terrain" => Some(MeshRenderOptions::terrain_patch()),
        "foliage_instanced" | "foliage" | "tree" => Some(MeshRenderOptions::foliage_instanced()),
        "character_body" | "character" | "player" => Some(MeshRenderOptions::character_body()),
        "first_person_view_model" | "view_model" | "fps_view_model" => {
            Some(MeshRenderOptions::first_person_view_model())
        }
        "sky_background" | "sky" => Some(MeshRenderOptions::sky_background()),
        "celestial_billboard" => Some(MeshRenderOptions::celestial_billboard()),
        _ => None,
    }
}

fn shadow_policy_from_string(value: &str) -> Option<MeshShadowPolicy> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "none" | "off" | "disabled" => Some(MeshShadowPolicy::None),
        "cast" | "cast_only" | "caster" => Some(MeshShadowPolicy::CastOnly),
        "receive" | "receive_only" | "receiver" | "recive" | "recive_only" => {
            Some(MeshShadowPolicy::ReceiveOnly)
        }
        "cast_and_receive" | "cast_receive" | "cast_and_recive" | "cast_recive" => {
            Some(MeshShadowPolicy::CastAndReceive)
        }
        "profile" | "profile_controlled" | "profiled" => Some(MeshShadowPolicy::ProfileControlled),
        _ => None,
    }
}

fn apply_shadow_policy_from_metadata(
    raw: &RawDefinitionEntryV1,
    mut options: MeshRenderOptions,
) -> MeshRenderOptions {
    if let Some(policy) = raw_render_string(raw, "shadow_policy")
        .or_else(|| raw_render_string(raw, "shadow.policy"))
        .or_else(|| raw_render_string(raw, "render.shadow_policy"))
        .or_else(|| raw_render_string(raw, "render.shadow.policy"))
        .and_then(|value| shadow_policy_from_string(&value))
    {
        options.shadow_policy = policy;
    }
    options
}

fn infer_render_options(raw: &RawDefinitionEntryV1, _refs: &DefinitionRefsV1) -> MeshRenderOptions {
    let options = if let Some(options) = raw_render_string(raw, "mesh.role")
        .or_else(|| raw_render_string(raw, "role"))
        .and_then(|role| render_options_from_role(&role))
    {
        options
    } else if raw_has_tag(raw, "sky") {
        MeshRenderOptions::sky_background()
    } else if raw_has_tag(raw, "terrain") {
        MeshRenderOptions::terrain_patch()
    } else if raw_has_tag(raw, "foliage") || raw_has_tag(raw, "tree") {
        MeshRenderOptions::foliage_instanced()
    } else if raw_has_tag(raw, "player") || raw_has_tag(raw, "character") {
        MeshRenderOptions::character_body()
    } else {
        MeshRenderOptions::world_opaque()
    };
    apply_shadow_policy_from_metadata(raw, options)
}

fn build_model_explanation(
    source: &str,
    raw: &RawDefinitionEntryV1,
    refs: &DefinitionRefsV1,
) -> ModelExplanationV1 {
    let drawable_ref = refs.drawable_refs.first().cloned();
    ModelExplanationV1 {
        source: source.to_owned(),
        model_ref: drawable_ref.clone(),
        drawable_ref,
        material_bindings: raw.material_bindings.clone(),
        material_refs: refs.material_refs.clone(),
        texture_refs: refs.texture_refs.clone(),
        uv_layout_refs: refs.uv_layout_refs.clone(),
        physics_refs: refs.physics_refs.clone(),
        collision_refs: refs.collision_refs.clone(),
        render_options: infer_render_options(raw, refs),
        collision_policy: raw_render_string(raw, "collision.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        uv_policy: raw_render_string(raw, "uv.policy").unwrap_or_else(|| "authored".to_owned()),
        physics_policy: raw_render_string(raw, "physics.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        lod_policy: raw_render_string(raw, "lod.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        streaming_policy: raw_render_string(raw, "streaming.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        explanation:
            "YTYP descriptor binds .ydd slots to materials and declares render/collision/LOD policy"
                .to_owned(),
        ..Default::default()
    }
}

fn build_entry(
    source: &str,
    raw: RawDefinitionEntryV1,
    inherited_warnings: &[String],
) -> Result<DefinitionEntryV1, String> {
    let name = raw.name.trim().to_owned();
    if name.is_empty() {
        return Err(".ytyp Definition Entry has empty identity.name".to_owned());
    }
    let side_effects = collect_side_effects(&raw).map_err(|errors| errors.join("; "))?;
    let stable_hash = effective_hash(&raw);
    let refs = collect_refs(&raw);
    let model_explanation = build_model_explanation(source, &raw, &refs);
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
        refs,
        model_explanation,
        side_effects,
        arbitrary_metadata: arbitrary_metadata(&raw),
        warnings: inherited_warnings.to_vec(),
        ..Default::default()
    })
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
            match load_entry(state, request) {
                Ok(entry) => {
                    let flattened_refs = flatten_refs(&entry.refs);
                    ok_json(DefinitionRefResolutionV1 {
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
            match load_entry(state, request) {
                Ok(entry) => ok_json(serde_json::json!({
                    "ok": true,
                    "gateway": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    "byte_owner": ENGINE_ASSET_SERVICE_ID,
                    "semantic_owner": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    "definition_ref": entry.identity.definition_ref,
                    "side_effect_policy": "declarative-only; allowed shape is {domain,effect,target}; imperative run_code/script/call/function fields are rejected",
                    "side_effects": entry.side_effects,
                    "domain_tags": entry.domain_tags,
                })),
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
        .post_json_result::<DefinitionRefRequest, DefinitionValidationV1, _>(definitions_method::VALIDATE_V1, |state, request| Ok(validate_entry(state, request)))
        .post_json_result::<DefinitionRefRequest, DefinitionRefResolutionV1, _>(definitions_method::RESOLVE_REFS_V1, |state, request| {
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
        })
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::DESCRIBE_SIDE_EFFECTS_V1, |state, request| {
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
        })
        .blob(definitions_method::INVOKE_JSON, invoke_json)
        .blob(definitions_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
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
                serde_json::json!([{ "domain": "engine.assets.models", "effect": "require_drawable", "target": "models/foo.ydd@body" }]),
            )]),
            ..Default::default()
        };
        let effects = collect_side_effects(&raw).unwrap();
        assert_eq!(effects[0].domain, "engine.assets.models");
    }

    #[test]
    fn refs_are_classified_by_extension() {
        let raw = RawDefinitionEntryV1 {
            name: "body".to_owned(),
            dependencies: vec![
                AssetDependencyRecordV1::new(
                    "models/foo.ydd@body",
                    "drawable",
                    "engine.assets.models",
                    true,
                ),
                AssetDependencyRecordV1::new(
                    "materials/foo.nemat@body",
                    "material",
                    "engine.assets.materials",
                    true,
                ),
                AssetDependencyRecordV1::new(
                    "textures/foo.ytd@diff",
                    "texture",
                    "engine.assets.textures",
                    true,
                ),
            ],
            ..Default::default()
        };
        let refs = collect_refs(&raw);
        assert_eq!(refs.drawable_refs, vec!["models/foo.ydd@body"]);
        assert_eq!(refs.material_refs, vec!["materials/foo.nemat@body"]);
        assert_eq!(refs.texture_refs, vec!["textures/foo.ytd@diff"]);
    }

    #[test]
    fn json_ytyp_dictionary_preserves_uv_layout_refs_and_arbitrary_strings() {
        let body = br#"{
            "schema": "newengine.ytyp.dictionary.v1",
            "entries": [
                {
                    "name": "sky_northstar_default",
                    "semantic_tags": ["sky"],
                    "dependencies": [
                        {
                            "reference": "layouts/sky.ytyd@skydome_uv",
                            "role": "uv_layout",
                            "domain": "engine.model",
                            "required": true
                        }
                    ],
                    "metadata": {
                        "newengine.game_ready": {
                            "sky": {
                                "mesh": "any authored mesh string",
                                "definition_ref": "any authored definition string"
                            }
                        },
                        "render": {
                            "role": "sky_background",
                            "uv.policy": "authored_ytyd"
                        }
                    }
                }
            ]
        }
        "#;
        let (entries, warnings) = parse_ytyp_json_document("definitions/sky.ytyp", body).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(warnings.iter().any(|warning| warning.contains("JSON")));
        let entry = build_entry("definitions/sky.ytyp", entries[0].clone(), &warnings).unwrap();
        assert_eq!(
            entry.refs.uv_layout_refs,
            vec!["layouts/sky.ytyd@skydome_uv"]
        );
        assert!(entry.model_explanation.render_options.is_sky_role());
        assert_eq!(entry.model_explanation.uv_policy, "authored_ytyd");
        let metadata = entry
            .arbitrary_metadata
            .get("metadata")
            .and_then(|value| value.get("newengine.game_ready"))
            .and_then(|value| value.get("sky"))
            .unwrap();
        assert_eq!(
            metadata.get("mesh").and_then(|value| value.as_str()),
            Some("any authored mesh string")
        );
    }
}
