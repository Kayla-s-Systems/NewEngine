#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material gateway and strict .nemat -> .ytd@entry material resolution.
//!
//! Runtime material gateway for `.nemat@entry -> .ytd@entry` resolution.
//! Source image names are importer inputs, never runtime texture refs.

use newengine_authored_xml as authored_xml;
use newengine_materials::{
    validate_authored_material_library, validate_material_texture_reference,
    AuthoredMaterialDescriptor, AuthoredMaterialLibrary, MaterialDescriptor,
    MaterialDescriptorLoadResponse, MaterialDomain, MaterialFlags, MaterialLoadRequest,
    MaterialLoadResponse, MaterialParamValue, MaterialsManifest, MaterialTextureBindings,
    MaterialTextureRefInfo, MaterialTextureRefRequest, MaterialValidationRequest,
    MaterialValidationResult, RenderMaterialPacket, ResolvedMaterialGraph, ShadingModel,
};
use newengine_model_domain_api::ModelMaterialBinding;
use newengine_model_import_obj::ModelMaterialSource;

pub fn material_binding(
    material_slot: &str,
    parsed: Option<&ModelMaterialSource>,
    _texture_dictionary: Option<&str>,
) -> ModelMaterialBinding {
    let mut descriptor = parsed
        .map(|mat| MaterialDescriptor {
            base_color: [mat.kd[0], mat.kd[1], mat.kd[2], mat.alpha],
            roughness: (1.0 - (mat.ns / 512.0).clamp(0.0, 0.9)).clamp(0.28, 0.92),
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS)
                .union(if mat.alpha < 0.99 { MaterialFlags::ALPHA_BLEND } else { MaterialFlags::NONE }),
            ..MaterialDescriptor::default()
        })
        .unwrap_or_default();
    descriptor.sanitize_in_place();

    let mut textures = MaterialTextureBindings::default();
    if let Some(texture) = parsed
        .and_then(|mat| mat.base_color_texture.as_deref())
        .and_then(strict_runtime_texture_ref)
    {
        textures.base_color_texture = Some(texture);
    }
    if let Some(texture) = parsed
        .and_then(|mat| mat.normal_texture.as_deref())
        .and_then(strict_runtime_texture_ref)
    {
        textures.normal_texture = Some(texture);
    }

    let fallback_color = descriptor.base_color;
    ModelMaterialBinding {
        slot: material_slot.to_owned(),
        descriptor,
        textures: textures.sanitized(),
        fallback_color,
        material_ref: None,
        resolution_policy: "runtime_strict_ydd_nemat_ytd_chain".to_owned(),
    }
}

/// Runtime material paths accept only already-authored `.ytd@entry` selectors.
///
/// Deriving a texture entry from `*.dds`, `*.png`, `*.jpg` or an OBJ/MTL source
/// filename is importer/migration tooling behavior. It is intentionally absent
/// from this runtime helper so model/material hot paths cannot silently stitch
/// source images into authored material graphs.
pub fn strict_runtime_texture_ref(path: &str) -> Option<String> {
    match validate_material_texture_reference(path) {
        Ok(reference) => Some(reference.canonical),
        Err(error) => {
            log::debug!(
                "materials.runtime: rejected non-runtime texture ref path='{}' reason='{}' policy='.ytd@entry only'",
                path,
                error
            );
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_texture_ref_rejects_raw_source_images() {
        assert!(strict_runtime_texture_ref("player/abigail/textures/hair_diff_000_a_uni.dds").is_none());
        assert!(strict_runtime_texture_ref("textures/foo.png").is_none());
        assert!(strict_runtime_texture_ref("textures/foo.jpg").is_none());
    }

    #[test]
    fn runtime_texture_ref_rejects_ytd_without_entry() {
        assert!(strict_runtime_texture_ref("textures/world.ytd").is_none());
    }

    #[test]
    fn runtime_texture_ref_accepts_ytd_entry() {
        assert_eq!(
            strict_runtime_texture_ref("textures/world.ytd@brick_albedo").as_deref(),
            Some("textures/world.ytd@brick_albedo")
        );
    }

    #[test]
    fn nemat_entry_selector_is_first_class() {
        let (path, selector) = split_nemat_selector("materials/world/garage.nemat@garage_door", None).unwrap();
        assert_eq!(path, "materials/world/garage.nemat");
        assert_eq!(selector, "garage_door");
    }

    #[test]
    fn nemat_without_entry_is_rejected() {
        let err = split_nemat_selector("materials/world/garage.nemat", None).unwrap_err();
        assert!(err.contains("@entry"));
    }

    #[test]
    fn material_library_payload_selects_entry() {
        let payload = br#"<?xml version="1.0" encoding="UTF-8"?>
<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
  <Material name="garage_door" shader="pbr.default">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
      <Texture slot="base_color" ref="textures/world/garage.ytd@garage_door_bc" />
    </Textures>
    <Params>
      <Param name="roughness" type="float" value="0.7" />
    </Params>
  </Material>
</NematMaterialLibrary>
"#;
        let material = decode_material_entry_payload(payload, "garage_door").unwrap();
        assert_eq!(material.name, "garage_door");
        assert!(material.textures.contains_key("base_color"));
    }
}

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_assets_api::{textures_method, ASSET_LIST_FILE_BODY_OUTPUT, ENGINE_ASSETS_TEXTURES_SERVICE_ID};
use newengine_materials::{
    method as material_method, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
    MATERIALS_BACKEND_CAPABILITY_ID, MATERIALS_SERVICE_ID, MATERIALS_SERVICE_METHODS,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use newengine_materials::api::material_id_from_name;

#[derive(Clone)]
pub struct MaterialAssetGatewayAdapter {
    client: AssetServiceClient,
    host: Option<HostApiV1>,
    descriptor_cache: Arc<Mutex<HashMap<String, MaterialDescriptorLoadResponse>>>,
    graph_cache: Arc<Mutex<HashMap<String, ResolvedMaterialGraph>>>,
}

impl MaterialAssetGatewayAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self { client, host: None, descriptor_cache: Arc::new(Mutex::new(HashMap::default())), graph_cache: Arc::new(Mutex::new(HashMap::default())) }
    }

    #[inline]
    pub fn with_client_and_host(client: AssetServiceClient, host: HostApiV1) -> Self {
        Self { client, host: Some(host), descriptor_cache: Arc::new(Mutex::new(HashMap::default())), graph_cache: Arc::new(Mutex::new(HashMap::default())) }
    }

    pub fn load_material(&self, request: &MaterialLoadRequest) -> Result<MaterialLoadResponse, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        if !source.to_ascii_lowercase().ends_with(&format!(".{}", newengine_asset_format_nef8::nemat::EXTENSION)) {
            return Err(format!("materials: expected provider-declared material library path, got '{source}'"));
        }
        log::debug!(
            "assets.materials.load_descriptor_v1: source='{}' selector='{}' output_kind='{}' policy='NEF8 body from engine.assets; material semantics stay in engine.assets.materials'",
            source,
            selector,
            ASSET_LIST_FILE_BODY_OUTPUT
        );
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: source.clone(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
            })
            .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' selector='{selector}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{e}'"))?;
        let material = decode_material_entry_payload(&bytes, &selector)
            .map_err(|e| format!("materials: decode .nemat library failed source='{source}' selector='{selector}' err='{e}'"))?;
        log::debug!(
            "assets.materials.load_descriptor_v1: decoded source='{}' selector='{}' texture_slots={} params={}",
            source,
            selector,
            material.textures.len(),
            material.params.len()
        );
        material_response_from_authored(&source, &selector, material)
    }

    #[inline]
    pub fn describe_texture_ref(&self, request: &MaterialTextureRefRequest) -> MaterialTextureRefInfo {
        self.validate_texture_ref_through_textures_gateway(&request.reference)
    }

    fn validate_texture_ref_through_textures_gateway(&self, reference: &str) -> MaterialTextureRefInfo {
        let mut info = MaterialTextureRefInfo::from_reference(reference);
        if !info.valid {
            return info;
        }

        let request = serde_json::json!({ "texture_ref": info.canonical });
        let payload = match serde_json::to_vec(&request) {
            Ok(payload) => payload,
            Err(e) => {
                info.valid = false;
                info.errors.push(format!("engine.assets.textures validation payload encode failed: {e}"));
                return info;
            }
        };
        let Some(host) = self.host.clone() else {
            info.valid = false;
            info.errors.push(format!(
                "engine.assets.textures validation requires a HostApiV1 supplied by the runtime gateway registry for '{}'",
                info.canonical
            ));
            return info;
        };
        let result = (host.call_service_v1)(
            abi_stable::std_types::RString::from(ENGINE_ASSETS_TEXTURES_SERVICE_ID),
            MethodName::from(textures_method::VALIDATE_REF_V1),
            Blob::from(payload),
        );
        let bytes = match result.into_result() {
            Ok(bytes) => bytes.into_vec(),
            Err(e) => {
                info.valid = false;
                info.errors.push(format!("engine.assets.textures validation unavailable for '{}': {}", info.canonical, e));
                return info;
            }
        };
        let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => value,
            Err(e) => {
                info.valid = false;
                info.errors.push(format!("engine.assets.textures validation returned non-json for '{}': {}", info.canonical, e));
                return info;
            }
        };
        if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            info.warnings.push("validated_by=engine.assets.textures".to_owned());
        } else {
            info.valid = false;
            let message = value
                .get("message")
                .or_else(|| value.get("diagnostic"))
                .and_then(|v| v.as_str())
                .unwrap_or("engine.assets.textures rejected texture ref");
            info.errors.push(message.to_owned());
        }
        info
    }


    pub fn load_descriptor(&self, request: &MaterialLoadRequest) -> Result<MaterialDescriptorLoadResponse, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        let cache_key = material_cache_key(&source, &selector);
        if let Ok(cache) = self.descriptor_cache.lock() {
            if let Some(cached) = cache.get(&cache_key).cloned() {
                log::debug!(
                    "assets.materials.load_descriptor_v1: cache hit source='{}' selector='{}' policy='decoded .nemat entry cache'",
                    source,
                    selector
                );
                return Ok(cached);
            }
        }

        let loaded = self.load_material(&MaterialLoadRequest { logical_path: format!("{source}@{selector}"), selector: None })?;
        let response = MaterialDescriptorLoadResponse { source: loaded.source, name: loaded.name, descriptor: loaded.descriptor, textures: loaded.textures };
        if let Ok(mut cache) = self.descriptor_cache.lock() {
            cache.insert(cache_key, response.clone());
        }
        Ok(response)
    }

    pub fn validate_material(&self, request: &MaterialValidationRequest) -> MaterialValidationResult {
        let mut result = MaterialValidationResult { source: request.logical_path.clone(), ..Default::default() };
        let loaded = match self.load_descriptor(&MaterialLoadRequest { logical_path: request.logical_path.clone(), selector: request.selector.clone() }) {
            Ok(value) => value,
            Err(err) => { result.errors.push(err); return result; }
        };
        result.source = loaded.source.clone();
        for texture in collect_texture_refs(&loaded.textures) {
            let info = self.validate_texture_ref_through_textures_gateway(texture);
            if !info.valid { result.errors.extend(info.errors); }
        }
        result.valid = result.errors.is_empty();
        result
    }

    pub fn resolve_graph(&self, request: &MaterialLoadRequest) -> Result<ResolvedMaterialGraph, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        let cache_key = material_cache_key(&source, &selector);
        if let Ok(cache) = self.graph_cache.lock() {
            if let Some(cached) = cache.get(&cache_key).cloned() {
                log::debug!(
                    "assets.materials.resolve_graph_v1: cache hit source='{}' selector='{}' texture_refs={} warnings={}",
                    source,
                    selector,
                    cached.texture_refs.len(),
                    cached.warnings.len()
                );
                return Ok(cached);
            }
        }

        let loaded = self.load_descriptor(&MaterialLoadRequest { logical_path: format!("{source}@{selector}"), selector: None })?;
        let mut graph = ResolvedMaterialGraph { source: loaded.source, name: loaded.name, descriptor: loaded.descriptor, textures: loaded.textures, ..Default::default() };
        for texture in collect_texture_refs(&graph.textures) {
            let info = self.validate_texture_ref_through_textures_gateway(texture);
            if !info.valid { graph.warnings.extend(info.errors.clone()); }
            graph.texture_refs.push(info);
        }
        log::debug!(
            "assets.materials.resolve_graph_v1: source='{}' texture_refs={} warnings={} cache='store'",
            graph.source,
            graph.texture_refs.len(),
            graph.warnings.len()
        );
        if let Ok(mut cache) = self.graph_cache.lock() {
            cache.insert(cache_key, graph.clone());
        }
        Ok(graph)
    }

    pub fn to_render_packet(&self, request: &MaterialLoadRequest) -> Result<RenderMaterialPacket, String> {
        let graph = self.resolve_graph(request)?;
        if graph.texture_refs.iter().any(|r| !r.valid) {
            return Err(format!("materials: cannot produce RenderMaterialPacket for '{}' because texture references are invalid", graph.source));
        }
        log::debug!(
            "assets.materials.to_render_packet_v1: source='{}' name='{}' packet_kind='renderer_agnostic_material_packet'",
            graph.source,
            graph.name
        );
        Ok(RenderMaterialPacket { source: graph.source, name: graph.name, descriptor: graph.descriptor, textures: graph.textures, ..Default::default() })
    }
}

#[derive(Clone)]
struct MaterialGatewayState {
    adapter: MaterialAssetGatewayAdapter,
}

#[derive(Clone, Debug, Serialize)]
pub struct MaterialsServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub native_formats: &'static [&'static str],
    pub texture_reference_policy: &'static str,
}

impl MaterialGatewayState {
    fn new(adapter: MaterialAssetGatewayAdapter) -> Self { Self { adapter } }


    fn formats_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "newengine.assets.materials.formats.v1",
            "gateway": ENGINE_ASSETS_MATERIALS_SERVICE_ID,
            "formats": [
                {
                    "extension": "nemat",
                    "asset_kind": "material_library",
                    "container": "newengine.listfile.nef8.nemat",
                    "read_method": material_method::LOAD_DESCRIPTOR_V1,
                    "resolve_method": material_method::RESOLVE_GRAPH_V1,
                    "packet_method": material_method::TO_RENDER_PACKET_V1,
                    "runtime_ready": true,
                    "selector_syntax": "<logical-path>.nemat@entry",
                    "notes": "Native NewEngine XML material library inside NEF8/ListFile. Entries bind .ytd@entry texture references and resolve through engine.assets.materials."
                }
            ],
            "body_presentation": "xml",
            "texture_reference_policy": "material textures must be VFS .ytd@entry dictionary selectors; source image paths are importer-only"
        })
    }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        #[derive(Deserialize)]
        struct InvokeEnvelope {
            method: String,
            #[serde(default)]
            request: serde_json::Value,
        }

        let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid invoke_json payload: {e}"))),
        };

        match envelope.method.as_str() {
            material_method::LOAD_JSON_V1 | material_method::LOAD_DESCRIPTOR_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid load request: {e}"))),
                };
                match self.adapter.load_descriptor(&request) {
                    Ok(value) => ok_json(value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            material_method::DESCRIBE_TEXTURE_REF_JSON_V1 => {
                let request = match serde_json::from_value::<MaterialTextureRefRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid texture ref request: {e}"))),
                };
                ok_json(self.adapter.describe_texture_ref(&request))
            }
            material_method::FORMATS_JSON_V1 | material_method::MANIFEST_JSON_V1 => ok_json(MaterialsManifest::default()),
            material_method::RESOLVE_GRAPH_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid resolve graph request: {e}"))),
                };
                match self.adapter.resolve_graph(&request) { Ok(value) => ok_json(value), Err(e) => RResult::RErr(RString::from(e)) }
            }
            material_method::VALIDATE_V1 => {
                let request = match serde_json::from_value::<MaterialValidationRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid validate request: {e}"))),
                };
                ok_json(self.adapter.validate_material(&request))
            }
            material_method::TO_RENDER_PACKET_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid render packet request: {e}"))),
                };
                match self.adapter.to_render_packet(&request) { Ok(value) => ok_json(value), Err(e) => RResult::RErr(RString::from(e)) }
            }
            other => RResult::RErr(RString::from(format!("materials.api: unknown invoke method '{other}'"))),
        }
    }
}

pub fn materials_service_info() -> MaterialsServiceInfo {
    MaterialsServiceInfo {
        id: MATERIALS_SERVICE_ID,
        gateway: ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        methods: MATERIALS_SERVICE_METHODS,
        backend: "engine-owned.material-runtime",
        native_formats: &[".nemat"],
        texture_reference_policy: ".ytd@entry dictionary selectors only for authored/runtime material graphs",
    }
}

pub fn materials_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    materials_gateway_service_with_host(client, None)
}

pub fn materials_gateway_service_with_host(
    client: AssetServiceClient,
    host: Option<HostApiV1>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        MATERIALS_SERVICE_ID,
        "newengine-material-runtime.material-gateway",
        MATERIALS_BACKEND_CAPABILITY_ID,
        MATERIALS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_MATERIALS_SERVICE_ID)
    .protocol("json")
    .features(["nemat-library", "nemat-entry-selectors", "ytd-texture-selectors", "render-material-packet"])
    .notes("Engine material gateway. Descriptors are read through engine.assets/VFS, resolved to material graphs, then converted to renderer-agnostic RenderMaterialPacket.");

    JsonServiceRouter::with_state(
        MATERIALS_SERVICE_ID,
        MaterialGatewayState::new(match host {
            Some(host) => MaterialAssetGatewayAdapter::with_client_and_host(client, host),
            None => MaterialAssetGatewayAdapter::with_client(client),
        }),
    )
    .describe_json(&description)
    .info(materials_service_info)
    .get_json(material_method::FORMATS_JSON_V1, |state| state.formats_json())
    .get_json(material_method::MANIFEST_JSON_V1, |_state| MaterialsManifest::default())
    .post_json_result::<MaterialLoadRequest, MaterialDescriptorLoadResponse, _>(
        material_method::LOAD_JSON_V1,
        |state, request| state.adapter.load_descriptor(&request),
    )
    .post_json_result::<MaterialLoadRequest, MaterialDescriptorLoadResponse, _>(
        material_method::LOAD_DESCRIPTOR_V1,
        |state, request| state.adapter.load_descriptor(&request),
    )
    .post_json_result::<MaterialLoadRequest, ResolvedMaterialGraph, _>(
        material_method::RESOLVE_GRAPH_V1,
        |state, request| state.adapter.resolve_graph(&request),
    )
    .post_json::<MaterialValidationRequest, MaterialValidationResult, _>(
        material_method::VALIDATE_V1,
        |state, request| state.adapter.validate_material(&request),
    )
    .post_json_result::<MaterialLoadRequest, RenderMaterialPacket, _>(
        material_method::TO_RENDER_PACKET_V1,
        |state, request| state.adapter.to_render_packet(&request),
    )
    .post_json::<MaterialTextureRefRequest, MaterialTextureRefInfo, _>(
        material_method::DESCRIBE_TEXTURE_REF_JSON_V1,
        |state, request| state.adapter.describe_texture_ref(&request),
    )
    .blob(material_method::INVOKE_JSON, |state, payload| state.invoke_json(payload))
    .blob(material_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
    .into_service_v1()
}

pub fn register_materials_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_materials_gateway_best_effort_with_host(None, client)
}

pub fn register_materials_gateway_best_effort_with_host(host: Option<HostApiV1>, client: AssetServiceClient) -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        service_kind: EngineServiceKind::Materials,
        provider_service: MATERIALS_SERVICE_ID,
        capability: MATERIALS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-material-runtime.material-gateway",
        service: materials_gateway_service_with_host(client, host),
    })
}


#[inline]
fn material_cache_key(source: &str, selector: &str) -> String {
    format!("{}@{}", source.trim().replace('\\', "/"), selector.trim())
}

#[inline]
fn split_nemat_selector(logical_path: &str, request_selector: Option<&str>) -> Result<(String, String), String> {
    let (path_part, selector_from_path) = match logical_path.rsplit_once('@') {
        Some((path, selector)) => (path.trim(), Some(selector.trim())),
        None => (logical_path.trim(), None),
    };
    let selector = request_selector
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(selector_from_path)
        .ok_or_else(|| format!("materials: .nemat material references must select an entry with @entry, got '{logical_path}'"))?;
    if selector.starts_with("hash:") {
        return Err(format!("materials: hash selector '{}' is reserved for the ListFile codec; material runtime requires the resolved entry name", selector));
    }
    if selector.contains('/') || selector.contains('\\') || selector.contains("..") {
        return Err(format!("materials: invalid .nemat entry selector '{selector}'"));
    }
    let source = normalize_material_logical_path(path_part)?;
    Ok((source, selector.to_owned()))
}

fn decode_material_entry_payload(bytes: &[u8], selector: &str) -> Result<AuthoredMaterialDescriptor, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "NEMAT payload must be UTF-8 XML material library inside the NEF8 ListFile body".to_owned())?;
    if !authored_xml::text_is_xml(text) {
        return Err("NEMAT body must be XML <NematMaterialLibrary>; JSON material bodies are forbidden in authored .nemat files".to_owned());
    }
    let library = decode_nemat_material_library_xml(text)?;
    let validation = validate_authored_material_library(&library);
    if !validation.valid {
        return Err(format!("invalid XML material library: {}", validation.errors.join("; ")));
    }
    let available = library
        .materials
        .iter()
        .map(|material| material.name.as_str())
        .filter(|name| !name.trim().is_empty())
        .collect::<Vec<_>>()
        .join(",");
    library
        .materials
        .into_iter()
        .find(|material| material.name.trim().eq_ignore_ascii_case(selector.trim()))
        .ok_or_else(|| format!("material entry '{selector}' not found in XML .nemat library; available=[{available}]"))
}

fn decode_nemat_material_library_xml(text: &str) -> Result<AuthoredMaterialLibrary, String> {
    let doc = authored_xml::parse_xml_document(text, "engine.assets.materials .nemat")?;
    let root = doc.root_element();
    if !authored_xml::root_has_any_name(root, &["NematMaterialLibrary", "MaterialLibrary"]) {
        return Err(format!(
            "NEMAT XML root must be <NematMaterialLibrary>, actual='{}'",
            root.tag_name().name()
        ));
    }
    let schema = authored_xml::root_schema(root);
    if !schema.is_empty() && schema != "newengine.nemat.material_library.v1" {
        return Err(format!("unsupported NEMAT XML schema '{schema}', expected 'newengine.nemat.material_library.v1'"));
    }
    let mut library = AuthoredMaterialLibrary {
        version: authored_xml::xml_attr_u32_any(root, &["version"]).unwrap_or(1),
        materials: Vec::new(),
    };
    for material_node in root.children().filter(|child| child.is_element() && child.has_tag_name("Material")) {
        library.materials.push(decode_nemat_material_xml(material_node)?);
    }
    Ok(library)
}

fn decode_nemat_material_xml(node: authored_xml::XmlNode<'_, '_>) -> Result<AuthoredMaterialDescriptor, String> {
    let mut material = AuthoredMaterialDescriptor {
        name: authored_xml::xml_attr_any(node, &["name", "id"]).unwrap_or_default(),
        shader: authored_xml::xml_attr_any(node, &["shader", "shader_id", "shaderId"]).unwrap_or_else(|| "pbr.default".to_owned()),
        ..AuthoredMaterialDescriptor::default()
    };
    if material.name.trim().is_empty() {
        return Err("NEMAT XML <Material> entry missing name".to_owned());
    }
    if let Some(surface) = authored_xml::xml_child(node, "Surface") {
        material.surface.blend = authored_xml::xml_attr_any(surface, &["blend"]).unwrap_or_else(|| "opaque".to_owned());
        material.surface.two_sided = authored_xml::xml_attr_bool_any(surface, &["two_sided", "twoSided", "double_sided", "doubleSided"]).unwrap_or(false);
        material.surface.alpha_cutoff = authored_xml::xml_attr_f32_any(surface, &["alpha_cutoff", "alphaCutoff"]);
    }
    if let Some(textures) = authored_xml::xml_child(node, "Textures") {
        for texture in textures.children().filter(|child| child.is_element() && child.has_tag_name("Texture")) {
            let slot = authored_xml::xml_attr_any(texture, &["slot", "name"]).unwrap_or_default();
            let reference = authored_xml::xml_attr_any(texture, &["ref", "reference", "texture_ref", "textureRef"]).unwrap_or_default();
            if !slot.trim().is_empty() && !reference.trim().is_empty() {
                material.textures.insert(slot, reference);
            }
        }
    }
    if let Some(params) = authored_xml::xml_child(node, "Params") {
        for param in params.children().filter(|child| child.is_element() && child.has_tag_name("Param")) {
            let name = authored_xml::xml_attr_any(param, &["name", "key"]).unwrap_or_default();
            if name.trim().is_empty() { continue; }
            let kind = authored_xml::xml_attr_any(param, &["type", "kind"]).unwrap_or_else(|| "float".to_owned());
            let raw = authored_xml::xml_attr_any(param, &["value", "ref", "reference"])
                .or_else(|| param.text().map(str::trim).filter(|v| !v.is_empty()).map(ToOwned::to_owned))
                .unwrap_or_default();
            material.params.insert(name, parse_material_param_value(&kind, &raw)?);
        }
    }
    Ok(material)
}

fn parse_material_param_value(kind: &str, raw: &str) -> Result<MaterialParamValue, String> {
    let kind = kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "float" | "f32" => Ok(MaterialParamValue::Float(parse_f32(raw)?)),
        "float2" | "vec2" => Ok(MaterialParamValue::Float2(parse_f32_array::<2>(raw)?)),
        "float3" | "vec3" => Ok(MaterialParamValue::Float3(parse_f32_array::<3>(raw)?)),
        "float4" | "vec4" => Ok(MaterialParamValue::Float4(parse_f32_array::<4>(raw)?)),
        "color" | "rgba" => Ok(MaterialParamValue::Color(parse_f32_array::<4>(raw)?)),
        "int" | "i32" => raw.trim().parse::<i32>().map(MaterialParamValue::Int).map_err(|e| format!("material int param parse failed value='{raw}' err='{e}'")),
        "bool" | "boolean" => Ok(MaterialParamValue::Bool(matches!(raw.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))),
        "enum" => Ok(MaterialParamValue::Enum(raw.trim().to_owned())),
        "texture_ref" | "texture" => Ok(MaterialParamValue::TextureRef(raw.trim().to_owned())),
        other => Err(format!("unsupported material XML param type '{other}' value='{raw}'")),
    }
}

fn parse_f32(raw: &str) -> Result<f32, String> {
    raw.trim().parse::<f32>().map_err(|e| format!("material float param parse failed value='{raw}' err='{e}'"))
}

fn parse_f32_array<const N: usize>(raw: &str) -> Result<[f32; N], String> {
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(parse_f32)
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != N {
        return Err(format!("material vector param expected {N} components, got {} value='{raw}'", values.len()));
    }
    let mut out = [0.0; N];
    out.copy_from_slice(&values);
    Ok(out)
}

fn material_response_from_authored(source: &str, selector: &str, material: AuthoredMaterialDescriptor) -> Result<MaterialLoadResponse, String> {
    let mut descriptor = descriptor_from_authored(&material);
    descriptor.sanitize_in_place();
    let textures = texture_bindings_from_authored(&material)?;
    let name = if material.name.trim().is_empty() { selector.to_owned() } else { material.name };
    Ok(MaterialLoadResponse {
        source: format!("{source}@{selector}"),
        id: material_id_from_name(&name),
        name,
        descriptor,
        textures,
    })
}

fn descriptor_from_authored(material: &AuthoredMaterialDescriptor) -> MaterialDescriptor {
    let mut descriptor = MaterialDescriptor::default();
    descriptor.domain = MaterialDomain::Surface;
    descriptor.shading_model = if material.shader.to_ascii_lowercase().contains("unlit") {
        ShadingModel::Unlit
    } else {
        ShadingModel::PbrMetallicRoughness
    };
    if material.surface.two_sided {
        descriptor.flags = descriptor.flags.union(MaterialFlags::DOUBLE_SIDED);
    }
    let blend = material.surface.blend.to_ascii_lowercase();
    if blend.contains("alpha") || blend.contains("blend") || blend == "transparent" {
        descriptor.flags = descriptor.flags.union(MaterialFlags::ALPHA_BLEND);
    }
    if let Some(alpha_cutoff) = material.surface.alpha_cutoff {
        descriptor.flags = descriptor.flags.union(MaterialFlags::ALPHA_TEST);
        descriptor.alpha_cutoff = alpha_cutoff;
    }
    if let Some(value) = param_f32(&material.params, "metallic") {
        descriptor.metallic = value;
    }
    if let Some(value) = param_f32(&material.params, "roughness") {
        descriptor.roughness = value;
    }
    if let Some(value) = param_f32(&material.params, "normal_scale") {
        descriptor.normal_scale = value;
    }
    if let Some(value) = param_f32(&material.params, "occlusion_strength") {
        descriptor.occlusion_strength = value;
    }
    if let Some(value) = param_f32(&material.params, "emissive_strength") {
        descriptor.emissive_strength = value;
    }
    if let Some(color) = param_color(&material.params, "base_color") {
        descriptor.base_color = color;
    }
    if let Some(color) = param_float3(&material.params, "emissive") {
        descriptor.emissive = color;
    }
    descriptor
}

fn texture_bindings_from_authored(material: &AuthoredMaterialDescriptor) -> Result<MaterialTextureBindings, String> {
    let mut bindings = MaterialTextureBindings::default();
    for (slot, reference) in &material.textures {
        let canonical = validate_material_texture_reference(reference)
            .map_err(|e| format!("material '{}' texture slot '{}' invalid: {}", material.name, slot, e))?
            .canonical;
        match slot.as_str() {
            "base_color" | "albedo" | "diffuse" => bindings.base_color_texture = Some(canonical),
            "normal" | "normal_map" => bindings.normal_texture = Some(canonical),
            "metallic" => bindings.metallic_texture = Some(canonical),
            "roughness" => bindings.roughness_texture = Some(canonical),
            "occlusion" | "ao" => bindings.occlusion_texture = Some(canonical),
            "emissive" => bindings.emissive_texture = Some(canonical),
            other => return Err(format!("material '{}' has unknown texture slot '{}'", material.name, other)),
        }
    }
    Ok(bindings.sanitized())
}

fn param_f32(params: &std::collections::BTreeMap<String, MaterialParamValue>, key: &str) -> Option<f32> {
    match params.get(key)? {
        MaterialParamValue::Float(value) => Some(*value),
        MaterialParamValue::Int(value) => Some(*value as f32),
        _ => None,
    }
}

fn param_color(params: &std::collections::BTreeMap<String, MaterialParamValue>, key: &str) -> Option<[f32; 4]> {
    match params.get(key)? {
        MaterialParamValue::Color(value) | MaterialParamValue::Float4(value) => Some(*value),
        MaterialParamValue::Float3(value) => Some([value[0], value[1], value[2], 1.0]),
        _ => None,
    }
}

fn param_float3(params: &std::collections::BTreeMap<String, MaterialParamValue>, key: &str) -> Option<[f32; 3]> {
    match params.get(key)? {
        MaterialParamValue::Float3(value) => Some(*value),
        MaterialParamValue::Color(value) | MaterialParamValue::Float4(value) => Some([value[0], value[1], value[2]]),
        _ => None,
    }
}

fn collect_texture_refs(textures: &MaterialTextureBindings) -> Vec<&str> {
    let mut out = Vec::new();
    if let Some(value) = textures.base_color_texture.as_deref() { out.push(value); }
    if let Some(value) = textures.normal_texture.as_deref() { out.push(value); }
    if let Some(value) = textures.metallic_texture.as_deref() { out.push(value); }
    if let Some(value) = textures.roughness_texture.as_deref() { out.push(value); }
    if let Some(value) = textures.occlusion_texture.as_deref() { out.push(value); }
    if let Some(value) = textures.emissive_texture.as_deref() { out.push(value); }
    out
}

fn normalize_material_logical_path(path: &str) -> Result<String, String> {
    let mut s = path.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    if s.is_empty() {
        return Err("materials: logical path is empty".to_owned());
    }
    Ok(s)
}
