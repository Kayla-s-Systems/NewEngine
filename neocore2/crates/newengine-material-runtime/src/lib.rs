#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material gateway and strict .nemat -> .ytd@entry material resolution.
//!
//! Importer compatibility may still project legacy OBJ/MTL material sources, but
//! runtime texture refs are never derived from DDS/PNG/JPG names here.

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
        let payload = br#"{
            "version": 1,
            "materials": [
                {
                    "name": "garage_door",
                    "shader": "pbr.default",
                    "surface": { "blend": "opaque", "two_sided": false, "alpha_cutoff": null },
                    "textures": { "base_color": "textures/world/garage.ytd@garage_door_bc" },
                    "params": { "roughness": { "type": "float", "value": 0.7 } }
                }
            ]
        }"#;
        let material = decode_material_entry_payload(payload, "garage_door").unwrap();
        assert_eq!(material.name, "garage_door");
        assert!(material.textures.contains_key("base_color"));
    }
}

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_materials::{
    method as material_method, ENGINE_MATERIALS_SERVICE_ID,
    MATERIALS_BACKEND_CAPABILITY_ID, MATERIALS_SERVICE_ID, MATERIALS_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use newengine_materials::api::material_id_from_name;

#[derive(Clone)]
pub struct MaterialAssetGatewayAdapter {
    client: AssetServiceClient,
}

impl MaterialAssetGatewayAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self { client }
    }

    pub fn load_material(&self, request: &MaterialLoadRequest) -> Result<MaterialLoadResponse, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        if !source.to_ascii_lowercase().ends_with(".nemat") {
            return Err(format!("materials: expected .nemat material library path, got '{source}'"));
        }
        log::debug!(
            "materials.load_descriptor_v1: source='{}' selector='{}' output_kind='material.raw'",
            source,
            selector
        );
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: source.clone(),
                output_kind: "material.raw".to_owned(),
                selector: serde_json::json!({ "entry": selector.clone() }),
            })
            .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' selector='{selector}' err='{e}'"))?;
        let material = decode_material_entry_payload(&bytes, &selector)
            .map_err(|e| format!("materials: decode .nemat library failed source='{source}' selector='{selector}' err='{e}'"))?;
        log::debug!(
            "materials.load_descriptor_v1: decoded source='{}' selector='{}' texture_slots={} params={}",
            source,
            selector,
            material.textures.len(),
            material.params.len()
        );
        material_response_from_authored(&source, &selector, material)
    }

    #[inline]
    pub fn describe_texture_ref(&self, request: &MaterialTextureRefRequest) -> MaterialTextureRefInfo {
        MaterialTextureRefInfo::from_reference(&request.reference)
    }


    pub fn load_descriptor(&self, request: &MaterialLoadRequest) -> Result<MaterialDescriptorLoadResponse, String> {
        let loaded = self.load_material(request)?;
        Ok(MaterialDescriptorLoadResponse { source: loaded.source, name: loaded.name, descriptor: loaded.descriptor, textures: loaded.textures })
    }

    pub fn validate_material(&self, request: &MaterialValidationRequest) -> MaterialValidationResult {
        let mut result = MaterialValidationResult { source: request.logical_path.clone(), ..Default::default() };
        let loaded = match self.load_descriptor(&MaterialLoadRequest { logical_path: request.logical_path.clone(), selector: request.selector.clone() }) {
            Ok(value) => value,
            Err(err) => { result.errors.push(err); return result; }
        };
        result.source = loaded.source.clone();
        for texture in collect_texture_refs(&loaded.textures) {
            let info = MaterialTextureRefInfo::from_reference(texture);
            if !info.valid { result.errors.extend(info.errors); }
        }
        result.valid = result.errors.is_empty();
        result
    }

    pub fn resolve_graph(&self, request: &MaterialLoadRequest) -> Result<ResolvedMaterialGraph, String> {
        let loaded = self.load_descriptor(request)?;
        let mut graph = ResolvedMaterialGraph { source: loaded.source, name: loaded.name, descriptor: loaded.descriptor, textures: loaded.textures, ..Default::default() };
        for texture in collect_texture_refs(&graph.textures) {
            let info = MaterialTextureRefInfo::from_reference(texture);
            if !info.valid { graph.warnings.extend(info.errors.clone()); }
            graph.texture_refs.push(info);
        }
        log::debug!(
            "materials.resolve_graph_v1: source='{}' texture_refs={} warnings={}",
            graph.source,
            graph.texture_refs.len(),
            graph.warnings.len()
        );
        Ok(graph)
    }

    pub fn to_render_packet(&self, request: &MaterialLoadRequest) -> Result<RenderMaterialPacket, String> {
        let graph = self.resolve_graph(request)?;
        if graph.texture_refs.iter().any(|r| !r.valid) {
            return Err(format!("materials: cannot produce RenderMaterialPacket for '{}' because texture references are invalid", graph.source));
        }
        log::debug!(
            "materials.to_render_packet_v1: source='{}' name='{}' packet_kind='renderer_agnostic_material_packet'",
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
            "schema": "newengine.materials.formats.v1",
            "gateway": ENGINE_MATERIALS_SERVICE_ID,
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
                    "notes": "Native NewEngine material library. Entries bind .ytd@entry texture references and resolve through engine.materials."
                }
            ],
            "texture_reference_policy": "material textures must be VFS .ytd@entry dictionary selectors; raw images and .neytd references are invalid"
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
        gateway: ENGINE_MATERIALS_SERVICE_ID,
        methods: MATERIALS_SERVICE_METHODS,
        backend: "engine-owned.material-runtime",
        native_formats: &[".nemat"],
        texture_reference_policy: ".ytd@entry dictionary selectors only; .neytd is invalid in authored/runtime material graphs",
    }
}

pub fn materials_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        MATERIALS_SERVICE_ID,
        "newengine-material-runtime.material-gateway",
        MATERIALS_BACKEND_CAPABILITY_ID,
        MATERIALS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_MATERIALS_SERVICE_ID)
    .protocol("json")
    .features(["nemat-library", "nemat-entry-selectors", "ytd-texture-selectors", "render-material-packet"])
    .notes("Engine material gateway. Descriptors are read through engine.assets/VFS, resolved to material graphs, then converted to renderer-agnostic RenderMaterialPacket.");

    JsonServiceRouter::with_state(
        MATERIALS_SERVICE_ID,
        MaterialGatewayState::new(MaterialAssetGatewayAdapter::with_client(client)),
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
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_MATERIALS_SERVICE_ID,
        service_kind: EngineServiceKind::Materials,
        provider_service: MATERIALS_SERVICE_ID,
        capability: MATERIALS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-material-runtime.material-gateway",
        service: materials_gateway_service(client),
    })
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
        .map_err(|_| "binary single-material NEMAT payload is no longer a runtime material-library body; expected material library JSON/domain entry payload from NEF8 .nemat".to_owned())?;

    if let Ok(library) = serde_json::from_str::<AuthoredMaterialLibrary>(text) {
        let validation = validate_authored_material_library(&library);
        if !validation.valid {
            return Err(format!("invalid material library: {}", validation.errors.join("; ")));
        }
        return library
            .materials
            .into_iter()
            .find(|material| material.name == selector)
            .ok_or_else(|| format!("material entry '{selector}' not found in .nemat library"));
    }

    if let Ok(material) = serde_json::from_str::<AuthoredMaterialDescriptor>(text) {
        if !material.name.is_empty() && material.name != selector {
            return Err(format!("selected material entry '{selector}' does not match payload entry '{}'", material.name));
        }
        return Ok(AuthoredMaterialDescriptor { name: selector.to_owned(), ..material });
    }

    Err("material.raw is neither AuthoredMaterialLibrary nor AuthoredMaterialDescriptor JSON; .nemat must use one material-library body model".to_owned())
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
