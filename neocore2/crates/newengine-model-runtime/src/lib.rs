#![forbid(unsafe_op_in_unsafe_fn)]

//! Gateway-backed drawable/model runtime.
//!
//! `engine.assets.models` owns only drawable/model semantics. Definition metadata is read
//! through `engine.assets.definitions`; dependency graph expansion is read through
//! `engine.assets.graph`. This crate still hosts the current engine-owned graph
//! provider implementation; definition decoding is not part of model API.

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetService, AssetServiceClient};
use newengine_model_domain_api::{
    attach_content_hash, attach_metadata_namespace, attach_node_warning, attach_vfs_source,
    finalize_graph, fnv1a64, normalize_asset_ref, push_manifest_dependency, split_asset_ref,
    AssetGraphResolver, AssetGraphVfsSource, DrawableDictionaryManifest,
    DrawableDictionaryRequest, ModelAssetBundle, ModelAssetRequest, ModelConstructionManifest,
    ModelConstructionValidation, ModelMaterialBinding, ModelMeshPart, ResolvedAssetGraphV2,
    DRAWABLE_DICTIONARY_EXTENSION, ENGINE_ASSETS_MODELS_SERVICE_ID, MODEL_BACKEND_CAPABILITY_ID,
    MODEL_FEATURE_DOMAINS, MODEL_SERVICE_ID, MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, MODEL_SERVICE_METHOD_INVOKE,
    MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1, MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
    MODEL_SERVICE_METHODS,
};
use newengine_model_import_obj::normalize_logical_path;
use newengine_model_skeleton_api::ModelSkeletonMetadata;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};
use newengine_math::Vec3;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Clone)]
pub struct ModelAssetAdapter {
    client: AssetServiceClient,
    host: Option<HostApiV1>,
}

impl ModelAssetAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self { Self { client, host: None } }

    #[inline]
    pub fn with_client_and_host(client: AssetServiceClient, host: HostApiV1) -> Self {
        Self { client, host: Some(host) }
    }

    pub fn load_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let request = self.resolve_request(request)?;
        let target_height = request.target_height.clamp(0.25, 3.0);
        let source_ref = normalize_logical_path(&request.model, true)?;
        let (source_path, selector) = split_model_selector(&source_ref);
        let texture_dictionary = request
            .texture_dictionary
            .as_deref()
            .map(|path| normalize_logical_path(path, false))
            .transpose()?
            .filter(|path| path.ends_with(&format!(".{}", newengine_asset_format_ytd::EXTENSION)));

        let skeleton = match request.skeleton.as_deref() {
            Some(path) => Some(self.load_skeleton_metadata(path, target_height, request.eye_height_ratio)?),
            None => None,
        };

        if has_extension(&source_path, DRAWABLE_DICTIONARY_EXTENSION) {
            let parts = self.load_ydd_runtime_parts(&source_path, selector.as_deref())?;
            let collisions = if request.collisions.is_empty() {
                newengine_model_collision_runtime::default_collisions_for_model(skeleton.as_ref(), target_height)
            } else {
                request.collisions.clone()
            };
            return Ok(ModelAssetBundle { source: source_ref, parts, skeleton, texture_dictionary, collisions });
        }

        let source = normalize_logical_path(&request.model, false)?;
        let obj_text = self.read_text(&source)?;
        let decoded = newengine_model_import_obj::decode_obj_with_mtl_loader(
            &source,
            &obj_text,
            target_height,
            |path| self.read_text(path).ok(),
        )?;

        let mut parts = Vec::with_capacity(decoded.parts.len());
        for part in decoded.parts {
            let material = newengine_material_runtime::material_binding(
                &part.material_slot,
                decoded.materials.get(&part.material_slot),
                texture_dictionary.as_deref(),
            );
            parts.push(ModelMeshPart { material_slot: part.material_slot, mesh: part.mesh, material });
        }

        let collisions = if request.collisions.is_empty() {
            newengine_model_collision_runtime::default_collisions_for_model(skeleton.as_ref(), target_height)
        } else {
            request.collisions.clone()
        };

        Ok(ModelAssetBundle { source, parts, skeleton, texture_dictionary, collisions })
    }

    pub fn load_manifest(&self, logical_path: &str) -> Result<ModelConstructionManifest, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let text = self.read_text(&source)?;
        serde_json::from_str::<ModelConstructionManifest>(&text)
            .map_err(|e| format!("model manifest parse failed path='{source}' err='{e}'"))
    }

    pub fn resolve_request(&self, request: &ModelAssetRequest) -> Result<ModelAssetRequest, String> {
        let Some(manifest_path) = request.manifest.as_deref() else { return Ok(request.clone()); };
        let manifest = self.load_manifest(manifest_path)?;
        let mut resolved = request.clone();
        if resolved.model.trim().is_empty() { resolved.model = manifest.model; }
        if resolved.skeleton.is_none() { resolved.skeleton = manifest.skeleton.map(|it| it.source); }
        if resolved.texture_dictionary.is_none() { resolved.texture_dictionary = manifest.material_set.texture_dictionary; }
        if resolved.collisions.is_empty() { resolved.collisions = manifest.collisions; }
        if (resolved.target_height - ModelAssetRequest::default().target_height).abs() < f32::EPSILON {
            resolved.target_height = manifest.target_height;
        }
        if (resolved.eye_height_ratio - ModelAssetRequest::default().eye_height_ratio).abs() < f32::EPSILON {
            resolved.eye_height_ratio = manifest.eye_height_ratio;
        }
        Ok(resolved)
    }

    pub fn validate_request(&self, request: &ModelAssetRequest) -> ModelConstructionValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let resolved = match self.resolve_request(request) {
            Ok(resolved) => Some(resolved),
            Err(e) => { errors.push(e); None }
        };
        if let Some(resolved) = resolved.as_ref() {
            if resolved.model.trim().is_empty() {
                errors.push("model asset path is empty after manifest resolution".to_owned());
            }
            if resolved.skeleton.is_none() {
                warnings.push("no skeleton source declared; runtime model will use mesh-only binding".to_owned());
            }
            if resolved.texture_dictionary.is_none() {
                warnings.push("no texture dictionary declared; graph/material resolution should provide .ytd refs".to_owned());
            }
        }
        ModelConstructionValidation { valid: errors.is_empty(), resolved, errors, warnings }
    }

    pub fn load_drawable_dictionary_manifest(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        self.decode_model_manifest(
            &request.source,
            request.selector.as_deref(),
            DRAWABLE_DICTIONARY_EXTENSION,
            "drawable.manifest_json",
            "ydd drawable dictionary manifest",
        )
    }

    pub fn resolve_drawable(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        self.load_drawable_dictionary_manifest(request)
    }

    fn decode_model_manifest<T>(
        &self,
        source: &str,
        selector: Option<&str>,
        extension: &str,
        output_kind: &str,
        label: &str,
    ) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let source = normalize_logical_path(source, false)?;
        if !has_extension(&source, extension) {
            return Err(format!("{label} requires .{} source, got '{source}'", extension.trim_start_matches('.')));
        }
        let bytes = self.client.decode_v1(&AssetDecodeRequest {
            logical_path: source.clone(),
            output_kind: output_kind.to_owned(),
            selector: selector
                .map(|selector| serde_json::json!({ "selector": selector, "entry": selector }))
                .unwrap_or(serde_json::Value::Null),
        })
        .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' output='{output_kind}' err='{e}'"))?;
        serde_json::from_slice::<T>(&bytes)
            .map_err(|e| format!("model.api: {label} decode returned invalid json path='{source}' err='{e}'"))
    }

    pub fn load_skeleton_metadata(
        &self,
        logical_path: &str,
        target_height: f32,
        eye_height_ratio: f32,
    ) -> Result<ModelSkeletonMetadata, String> {
        let source = normalize_logical_path(logical_path, true)?;
        self.load_nef8_ymt_skeleton_metadata(&source, target_height, eye_height_ratio)
    }

    fn load_ydd_runtime_parts(&self, source: &str, selector: Option<&str>) -> Result<Vec<ModelMeshPart>, String> {
        let bytes = self.client.decode_v1(&AssetDecodeRequest {
            logical_path: source.to_owned(),
            output_kind: newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' output='{}' err='{e}'", newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT))?;
        let root: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("model.api: .ydd NEF8 body returned invalid json path='{source}' err='{e}'"))?;
        let encoding = root
            .get("mesh_encoding")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if encoding != "newengine.ydd.runtime_mesh_parts.v1" {
            return Err(format!("model.api: .ydd runtime mesh encoding unsupported path='{source}' encoding='{encoding}'"));
        }
        let parts = root
            .get("runtime_mesh_parts")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("model.api: .ydd has no runtime_mesh_parts array path='{source}'"))?;
        let mut out = Vec::new();
        for part in parts {
            let entry = part.get("entry").or_else(|| part.get("name")).and_then(serde_json::Value::as_str).unwrap_or_default();
            if selector.map(|needle| !needle.eq_ignore_ascii_case(entry)).unwrap_or(false) {
                continue;
            }
            out.push(self.decode_ydd_runtime_model_part(source, part)?);
        }
        if out.is_empty() {
            return Err(format!("model.api: .ydd selector '{}' produced no runtime mesh parts path='{source}'", selector.unwrap_or("<all>")));
        }
        Ok(out)
    }

    fn decode_ydd_runtime_model_part(&self, source: &str, part: &serde_json::Value) -> Result<ModelMeshPart, String> {
        let material_slot = part
            .get("material_slot")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("material")
            .trim()
            .to_owned();
        let vertices_json = part
            .get("vertices")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("model.api: .ydd runtime part has no vertices path='{source}' slot='{material_slot}'"))?;
        let indices_json = part
            .get("indices")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("model.api: .ydd runtime part has no indices path='{source}' slot='{material_slot}'"))?;
        let mut vertices = Vec::with_capacity(vertices_json.len());
        for (index, vertex) in vertices_json.iter().enumerate() {
            vertices.push(PrimitiveVertex {
                pos: json_vec3(vertex.get("pos"), source, index, "pos")?,
                nrm: json_vec3(vertex.get("nrm"), source, index, "nrm")?,
                uv: json_vec2(vertex.get("uv"), source, index, "uv")?,
            });
        }
        let mut indices = Vec::with_capacity(indices_json.len());
        for (index, value) in indices_json.iter().enumerate() {
            let item = value
                .as_u64()
                .ok_or_else(|| format!("model.api: .ydd index must be u32 path='{source}' slot='{material_slot}' index={index}"))?;
            let item = u32::try_from(item)
                .map_err(|_| format!("model.api: .ydd index exceeds u32 path='{source}' slot='{material_slot}' index={index}"))?;
            if item as usize >= vertices.len() {
                return Err(format!("model.api: .ydd index out of bounds path='{source}' slot='{material_slot}' index={item} vertices={}", vertices.len()));
            }
            indices.push(item);
        }
        let bounds_center = part
            .get("bounds_center")
            .map(|value| json_vec3(Some(value), source, 0, "bounds_center"))
            .transpose()?
            .map(|v| Vec3::new(v[0], v[1], v[2]))
            .unwrap_or(Vec3::ZERO);
        let bounds_radius = part
            .get("bounds_radius")
            .and_then(serde_json::Value::as_f64)
            .map(|value| value as f32)
            .unwrap_or_else(|| recompute_bounds_radius(bounds_center, &vertices));
        let material_ref = part
            .get("material_ref")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned);
        let material = match material_ref.as_deref().and_then(|material_ref| self.load_material_binding_from_ref(material_ref)) {
            Some(mut binding) => {
                binding.slot = material_slot.clone();
                binding
            }
            None => ModelMaterialBinding {
                slot: material_slot.clone(),
                material_ref,
                fallback_color: part
                    .get("fallback_color")
                    .and_then(|value| json_vec4_value(value).ok())
                    .unwrap_or([0.82, 0.78, 0.72, 1.0]),
                ..ModelMaterialBinding::default()
            },
        };
        Ok(ModelMeshPart {
            material_slot,
            mesh: PrimitiveMesh { vertices, indices, bounds_center, bounds_radius },
            material,
        })
    }

    fn load_nef8_ymt_skeleton_metadata(
        &self,
        source: &str,
        target_height: f32,
        eye_height_ratio: f32,
    ) -> Result<ModelSkeletonMetadata, String> {
        let path = source.split('@').next().unwrap_or(source);
        if !path.to_ascii_lowercase().ends_with(&format!(".{}", newengine_asset_format_ymt::EXTENSION)) {
            return Err(format!("model skeleton metadata requires provider-declared NEF8 skeleton metadata source, got '{source}'"));
        }
        let bytes = self.client.decode_v1(&AssetDecodeRequest {
            logical_path: path.to_owned(),
            output_kind: newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|e| format!("engine.assets decode_v1 failed path='{path}' output='{}' err='{e}'", newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT))?;
        let hash = format!("fnv1a64:{:016x}", fnv1a64(&bytes));
        Ok(ModelSkeletonMetadata {
            source: source.to_owned(),
            source_format: "newengine.ymt.metadata.v1".to_owned(),
            container_magic: "NEF8".to_owned(),
            byte_len: bytes.len(),
            content_hash: hash,
            decode_status: "metadata-only skeleton anchors generated from model target height".to_owned(),
            joints: default_humanoid_joints(target_height),
            anchors: default_humanoid_anchors(target_height, eye_height_ratio),
        })
    }

    fn read_bytes(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        let path = normalize_logical_path(logical_path, false)?;
        self.client
            .raw_bytes_v1(&path)
            .map_err(|e| format!("asset.raw_bytes_v1 failed path='{path}' err='{e}'"))
    }

    fn read_text(&self, logical_path: &str) -> Result<String, String> {
        let path = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&path)?;
        String::from_utf8(bytes).map_err(|e| format!("asset text is not UTF-8 path='{path}' err='{e}'"))
    }
}

fn split_model_selector(source: &str) -> (String, Option<String>) {
    match source.rsplit_once('@') {
        Some((path, selector)) => (path.to_owned(), Some(selector.to_owned()).filter(|it| !it.trim().is_empty())),
        None => (source.to_owned(), None),
    }
}

fn json_vec3(value: Option<&serde_json::Value>, source: &str, index: usize, label: &str) -> Result<[f32; 3], String> {
    let arr = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("model.api: .ydd vertex field '{label}' must be vec3 path='{source}' vertex={index}"))?;
    if arr.len() != 3 {
        return Err(format!("model.api: .ydd field '{label}' must have 3 components path='{source}' vertex={index} got={}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or_else(|| format!("model.api: .ydd '{label}.x' must be number path='{source}' vertex={index}"))? as f32,
        arr[1].as_f64().ok_or_else(|| format!("model.api: .ydd '{label}.y' must be number path='{source}' vertex={index}"))? as f32,
        arr[2].as_f64().ok_or_else(|| format!("model.api: .ydd '{label}.z' must be number path='{source}' vertex={index}"))? as f32,
    ])
}

fn json_vec2(value: Option<&serde_json::Value>, source: &str, index: usize, label: &str) -> Result<[f32; 2], String> {
    let arr = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("model.api: .ydd vertex field '{label}' must be vec2 path='{source}' vertex={index}"))?;
    if arr.len() != 2 {
        return Err(format!("model.api: .ydd field '{label}' must have 2 components path='{source}' vertex={index} got={}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or_else(|| format!("model.api: .ydd '{label}.x' must be number path='{source}' vertex={index}"))? as f32,
        arr[1].as_f64().ok_or_else(|| format!("model.api: .ydd '{label}.y' must be number path='{source}' vertex={index}"))? as f32,
    ])
}

fn json_vec4_value(value: &serde_json::Value) -> Result<[f32; 4], String> {
    let arr = value.as_array().ok_or_else(|| "vec4 value must be array".to_owned())?;
    if arr.len() != 4 { return Err(format!("vec4 value must have 4 components, got {}", arr.len())); }
    Ok([
        arr[0].as_f64().unwrap_or(1.0) as f32,
        arr[1].as_f64().unwrap_or(1.0) as f32,
        arr[2].as_f64().unwrap_or(1.0) as f32,
        arr[3].as_f64().unwrap_or(1.0) as f32,
    ])
}

fn recompute_bounds_radius(center: Vec3, vertices: &[PrimitiveVertex]) -> f32 {
    vertices
        .iter()
        .map(|v| {
            let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
            (p - center).length()
        })
        .fold(0.001, f32::max)
}

impl ModelAssetAdapter {
    fn load_material_binding_from_ref(&self, material_ref: &str) -> Option<ModelMaterialBinding> {
        let host = self.host.as_ref()?;
        let request = newengine_materials::MaterialLoadRequest { logical_path: material_ref.to_owned(), selector: None };
        let payload = serde_json::to_vec(&request).ok()?;
        let bytes = (host.call_service_v1)(
            RString::from(newengine_materials::ENGINE_ASSETS_MATERIALS_SERVICE_ID),
            MethodName::from(newengine_materials::method::LOAD_DESCRIPTOR_V1),
            Blob::from(payload),
        ).into_result().ok()?.into_vec();
        let response: newengine_materials::MaterialDescriptorLoadResponse = serde_json::from_slice(&bytes).ok()?;
        Some(ModelMaterialBinding {
            slot: response.name.clone(),
            material_ref: Some(material_ref.to_owned()),
            descriptor: response.descriptor,
            textures: response.textures,
            fallback_color: [0.82, 0.78, 0.72, 1.0],
            resolution_policy: "runtime_strict_ydd_nemat_ytd_chain".to_owned(),
        })
    }
}

fn default_humanoid_joints(target_height: f32) -> Vec<newengine_model_skeleton_api::ModelSkeletonJointMetadata> {
    use newengine_model_skeleton_api::skeleton_joint;
    vec![
        skeleton_joint("root", Option::<String>::None, [0.0, 0.0, 0.0]),
        skeleton_joint("hips", Some("root"), [0.0, target_height * 0.50, 0.0]),
        skeleton_joint("spine", Some("hips"), [0.0, target_height * 0.68, 0.0]),
        skeleton_joint("head", Some("spine"), [0.0, target_height * 0.91, 0.0]),
        skeleton_joint("left_hand", Some("spine"), [-0.42, target_height * 0.58, 0.0]),
        skeleton_joint("right_hand", Some("spine"), [0.42, target_height * 0.58, 0.0]),
        skeleton_joint("left_foot", Some("hips"), [-0.16, 0.02, 0.0]),
        skeleton_joint("right_foot", Some("hips"), [0.16, 0.02, 0.0]),
        skeleton_joint("eye", Some("head"), [0.0, target_height * 0.91, -0.08]),
    ]
}

fn default_humanoid_anchors(target_height: f32, eye_height_ratio: f32) -> newengine_model_skeleton_api::ModelSkeletonAnchors {
    newengine_model_skeleton_api::ModelSkeletonAnchors {
        root: "root".to_owned(),
        hips: "hips".to_owned(),
        head: "head".to_owned(),
        left_hand: "left_hand".to_owned(),
        right_hand: "right_hand".to_owned(),
        left_foot: "left_foot".to_owned(),
        right_foot: "right_foot".to_owned(),
        eye: "eye".to_owned(),
        eye_height: target_height * eye_height_ratio.clamp(0.55, 0.98),
    }
}

fn has_extension(path: &str, extension: &str) -> bool {
    let expected = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    path.split('@').next().unwrap_or(path).to_ascii_lowercase().ends_with(&format!(".{expected}"))
}

#[derive(Clone)]
pub struct ModelGatewayClient {
    host: HostApiV1,
    service_id: RString,
}

impl ModelGatewayClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self { Self { host, service_id: RString::from(ENGINE_ASSETS_MODELS_SERVICE_ID) } }

    #[inline]
    pub fn with_service_id(host: HostApiV1, service_id: &str) -> Self { Self { host, service_id: RString::from(service_id) } }

    pub fn assemble_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1, payload)?;
        serde_json::from_slice::<ModelAssetBundle>(&bytes)
            .map_err(|e| format!("engine.assets.models returned invalid ModelAssetBundle JSON: {e}"))
    }

    pub fn validate_request(&self, request: &ModelAssetRequest) -> Result<ModelConstructionValidation, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_VALIDATE_JSON_V1, payload)?;
        serde_json::from_slice::<ModelConstructionValidation>(&bytes)
            .map_err(|e| format!("engine.assets.models returned invalid validation JSON: {e}"))
    }

    pub fn drawable_dictionary_manifest(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, payload)?;
        serde_json::from_slice::<DrawableDictionaryManifest>(&bytes)
            .map_err(|e| format!("engine.assets.models returned invalid drawable dictionary manifest JSON: {e}"))
    }

    pub fn resolve_drawable(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1, payload)?;
        serde_json::from_slice::<DrawableDictionaryManifest>(&bytes)
            .map_err(|e| format!("engine.assets.models returned invalid resolved drawable JSON: {e}"))
    }

    fn call_raw(&self, method_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), MethodName::from(method_name), Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())
    }
}

#[derive(Clone)]
struct ModelRuntimeState { adapter: ModelAssetAdapter }

#[derive(Clone, Debug, Serialize)]
pub struct ModelRuntimeServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub feature_domains: &'static [&'static str],
}

impl ModelRuntimeState {
    fn new(adapter: ModelAssetAdapter) -> Self { Self { adapter } }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        #[derive(Deserialize)]
        struct InvokeEnvelope { method: String, #[serde(default)] request: serde_json::Value }
        let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid invoke_json payload: {e}"))),
        };
        match envelope.method.as_str() {
            MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid assemble request: {e}"))),
                };
                match self.adapter.load_bundle(&request) { Ok(bundle) => ok_json(bundle), Err(e) => RResult::RErr(RString::from(e)) }
            }
            MODEL_SERVICE_METHOD_VALIDATE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid validate request: {e}"))),
                };
                ok_json(self.adapter.validate_request(&request))
            }
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1 | MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1 => {
                let request = match serde_json::from_value::<DrawableDictionaryRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid drawable request: {e}"))),
                };
                match self.adapter.resolve_drawable(&request) { Ok(manifest) => ok_json(manifest), Err(e) => RResult::RErr(RString::from(e)) }
            }
            other => RResult::RErr(RString::from(format!("model.api: unknown invoke method '{other}'"))),
        }
    }
}

pub fn model_runtime_service_info() -> ModelRuntimeServiceInfo {
    ModelRuntimeServiceInfo {
        id: MODEL_SERVICE_ID,
        gateway: ENGINE_ASSETS_MODELS_SERVICE_ID,
        methods: MODEL_SERVICE_METHODS,
        backend: "engine-owned.model-runtime",
        feature_domains: MODEL_FEATURE_DOMAINS,
    }
}

pub fn model_gateway_service(adapter: ModelAssetAdapter) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        MODEL_SERVICE_ID,
        "newengine-model-runtime.model-gateway",
        MODEL_BACKEND_CAPABILITY_ID,
        MODEL_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_MODELS_SERVICE_ID)
    .protocol("json")
    .features(MODEL_FEATURE_DOMAINS.iter().copied())
    .notes("Gateway-backed drawable/model service. Definition metadata is owned by engine.assets.definitions; asset graph expansion is owned by engine.assets.graph.");

    JsonServiceRouter::with_state(MODEL_SERVICE_ID, ModelRuntimeState::new(adapter))
        .describe_json(&description)
        .info(model_runtime_service_info)
        .blob(MODEL_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_json(payload))
        .post_json_result::<ModelAssetRequest, ModelAssetBundle, _>(MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1, |state, request| state.adapter.load_bundle(&request))
        .post_json::<ModelAssetRequest, ModelConstructionValidation, _>(MODEL_SERVICE_METHOD_VALIDATE_JSON_V1, |state, request| state.adapter.validate_request(&request))
        .post_json_result::<DrawableDictionaryRequest, DrawableDictionaryManifest, _>(MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, |state, request| state.adapter.load_drawable_dictionary_manifest(&request))
        .post_json_result::<DrawableDictionaryRequest, DrawableDictionaryManifest, _>(MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1, |state, request| state.adapter.resolve_drawable(&request))
        .blob(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_model_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_model_gateway_service(client, None)
}

pub fn register_model_gateway_best_effort_with_host(host: HostApiV1, client: AssetServiceClient) -> bool {
    register_model_gateway_service(client, Some(host))
}

fn register_model_gateway_service(client: AssetServiceClient, host: Option<HostApiV1>) -> bool {
    let adapter = match host {
        Some(host) => ModelAssetAdapter::with_client_and_host(client, host),
        None => ModelAssetAdapter::with_client(client),
    };
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_ASSETS_MODELS_SERVICE_ID,
        service_kind: EngineServiceKind::Model,
        provider_service: MODEL_SERVICE_ID,
        capability: MODEL_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-model-runtime.model-gateway",
        service: model_gateway_service(adapter),
    })
}

#[derive(Clone)]
struct AssetGraphGatewayState {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl AssetGraphGatewayState {
    fn resolve(&self, root_ref: &str) -> ResolvedAssetGraphV2 {
        RuntimeAssetGraphResolver::new(self.host.clone(), self.client.clone()).resolve(root_ref)
    }
}

struct RuntimeAssetGraphResolver {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl RuntimeAssetGraphResolver {
    fn new(host: HostApiV1, client: AssetServiceClient) -> Self { Self { host, client } }

    fn resolve(&self, root_ref: &str) -> ResolvedAssetGraphV2 {
        let root_ref = normalize_asset_ref(root_ref);
        let mut graph = AssetGraphResolver::resolve_root_ref(&root_ref);
        graph.debug_log.push(format!("assets.graph.resolve_v1: hydration begin root_ref='{root_ref}'"));
        let mut visiting = Vec::<String>::new();
        let mut visited = std::collections::BTreeSet::<String>::new();
        self.resolve_ref(&mut graph, &root_ref, &mut visiting, &mut visited);
        finalize_graph(&mut graph);
        graph
    }

    fn resolve_ref(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
        visiting: &mut Vec<String>,
        visited: &mut std::collections::BTreeSet<String>,
    ) {
        let asset_ref = normalize_asset_ref(asset_ref);
        if asset_ref.is_empty() { return; }
        if visiting.iter().any(|item| item == &asset_ref) {
            let mut cycle = visiting.clone();
            cycle.push(asset_ref.clone());
            graph.cycle_errors.push(cycle.join(" -> "));
            return;
        }
        if !visited.insert(asset_ref.clone()) { return; }
        visiting.push(asset_ref.clone());
        self.attach_source_and_hash(graph, &asset_ref);

        let deps = match extension_of_ref(&asset_ref).as_deref() {
            Some("ytyp") => self.resolve_ytyp_entry(graph, &asset_ref),
            Some("ydd") => self.resolve_ydd_manifest(graph, &asset_ref),
            Some("nemat") => self.resolve_nemat_graph(graph, &asset_ref),
            Some("ytd") => self.validate_ytd_ref(graph, &asset_ref),
            Some("ymap") | Some("ymf") | Some("ymt") | Some("ywr") | Some("ysc")
            | Some("ybn") | Some("ybd") | Some("ycol") | Some("ydr") | Some("yft")
            | Some("ycd") | Some("yed") | Some("yfd") | Some("yld") | Some("ypdb")
            | Some("yvr") | Some("ytf") => self.resolve_generic_manifest(graph, &asset_ref, "listfile_dependency"),
            Some("nebrain") | Some("nepat") | Some("nemem") | Some("negoal") | Some("nebt")
            | Some("nebehavior") | Some("neutility") | Some("nebb") => self.resolve_generic_manifest(graph, &asset_ref, "ai_dependency"),
            Some(other) => {
                graph.format_warnings.push(format!("assets.graph.resolve_v1: no semantic resolver for ref='{asset_ref}' extension='.{other}'"));
                Vec::new()
            }
            None => Vec::new(),
        };

        for (dep, role, required) in deps {
            push_manifest_dependency(graph, &asset_ref, &dep, &role, required);
            self.resolve_ref(graph, &dep, visiting, visited);
        }
        visiting.pop();
    }

    fn attach_source_and_hash(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) {
        let (path, _) = split_asset_ref(asset_ref);
        if path.is_empty() { return; }
        match self.client.raw_bytes_v1(&path) {
            Ok(bytes) => attach_content_hash(graph, asset_ref, format!("fnv1a64:{:016x}", fnv1a64(&bytes))),
            Err(err) => {
                graph.missing_refs.push(format!("{asset_ref}: VFS bytes unavailable: {err}"));
                attach_node_warning(graph, asset_ref, format!("VFS bytes unavailable: {err}"));
            }
        }
        match self.client.resolve_trace_json_v1(&path) {
            Ok(trace) => attach_vfs_source(graph, asset_ref, vfs_source_from_trace(&path, &trace)),
            Err(err) => {
                attach_node_warning(graph, asset_ref, format!("VFS source trace unavailable: {err}"));
                attach_vfs_source(graph, asset_ref, AssetGraphVfsSource { source_kind: "unresolved".to_owned(), logical_path: path, ..Default::default() });
            }
        }
    }

    fn resolve_ytyp_entry(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) -> Vec<(String, String, bool)> {
        let request = serde_json::json!({ "definition_ref": asset_ref });
        match self.call_gateway_json(newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, newengine_assets_api::definitions_method::ENTRY_JSON_V1, request) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                // Do not scrape arbitrary strings out of `.ytyp` metadata. A
                // Definition Entry may mention a sky mesh, player model or editor
                // asset as descriptive knowledge; that does not make it a spawn
                // command. engine.assets.definitions owns the semantic ref projection and
                // provides explicit buckets. AssetGraph preserves those as graph
                // dependencies only; scene/apply systems decide what to instantiate.
                definition_entry_refs_to_edges(value.get("refs"), asset_ref)
            }
            Err(err) => {
                graph.missing_refs.push(format!("{asset_ref}: assets.definitions.entry_v1 failed: {err}"));
                attach_node_warning(graph, asset_ref, format!("assets.definitions.entry_v1 failed: {err}"));
                Vec::new()
            }
        }
    }

    fn resolve_ydd_manifest(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) -> Vec<(String, String, bool)> {
        let (source, selector) = split_asset_ref(asset_ref);
        let request = serde_json::json!({ "source": source, "selector": selector });
        match self.call_gateway_json(ENGINE_ASSETS_MODELS_SERVICE_ID, MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, request) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                let mut deps = collect_ref_strings(&value);
                deps.retain(|dep| dep != asset_ref);
                refs_to_edges(deps, "drawable_dependency")
            }
            Err(err) => {
                graph.missing_refs.push(format!("{asset_ref}: assets.models.drawable_manifest_v1 failed: {err}"));
                attach_node_warning(graph, asset_ref, format!("assets.models.drawable_manifest_v1 failed: {err}"));
                Vec::new()
            }
        }
    }

    fn resolve_nemat_graph(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) -> Vec<(String, String, bool)> {
        let request = serde_json::json!({ "logical_path": asset_ref, "selector": serde_json::Value::Null });
        match self.call_gateway_json(newengine_materials::ENGINE_ASSETS_MATERIALS_SERVICE_ID, newengine_materials::method::RESOLVE_GRAPH_V1, request) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                let mut deps = collect_ref_strings(&value);
                deps.retain(|dep| dep != asset_ref);
                refs_to_edges(deps, "material_texture")
            }
            Err(err) => {
                graph.missing_refs.push(format!("{asset_ref}: assets.materials.resolve_graph_v1 failed: {err}"));
                attach_node_warning(graph, asset_ref, format!("assets.materials.resolve_graph_v1 failed: {err}"));
                Vec::new()
            }
        }
    }

    fn validate_ytd_ref(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) -> Vec<(String, String, bool)> {
        let (path, selector) = split_asset_ref(asset_ref);
        if selector.is_none() {
            let request = AssetDecodeRequest {
                logical_path: path.clone(),
                output_kind: newengine_assets_api::method::LIST_FILE_MANIFEST_V1.to_owned(),
                selector: serde_json::Value::Null,
            };
            if let Err(err) = self.client.decode_v1(&request) {
                graph.missing_refs.push(format!("{asset_ref}: texture dictionary manifest unavailable: {err}"));
                attach_node_warning(graph, asset_ref, format!("texture dictionary manifest unavailable: {err}"));
            }
            return Vec::new();
        }
        let request = serde_json::json!({ "texture_ref": asset_ref });
        if let Err(err) = self.call_gateway_json(newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID, newengine_assets_api::textures_method::VALIDATE_REF_V1, request) {
            graph.missing_refs.push(format!("{asset_ref}: assets.textures.validate_ref_v1 failed: {err}"));
            attach_node_warning(graph, asset_ref, format!("assets.textures.validate_ref_v1 failed: {err}"));
        }
        Vec::new()
    }

    fn resolve_generic_manifest(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str, role: &str) -> Vec<(String, String, bool)> {
        let (path, _) = split_asset_ref(asset_ref);
        let request = AssetDecodeRequest { logical_path: path.clone(), output_kind: newengine_assets_api::method::LIST_FILE_MANIFEST_V1.to_owned(), selector: serde_json::Value::Null };
        match self.client.decode_v1(&request) {
            Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(value) => refs_to_edges(collect_ref_strings(&value), role),
                Err(err) => {
                    graph.metadata_warnings.push(format!("{asset_ref}: generic manifest decode returned non-json: {err}"));
                    Vec::new()
                }
            },
            Err(err) => {
                attach_node_warning(graph, asset_ref, format!("generic manifest unavailable: {err}"));
                Vec::new()
            }
        }
    }

    fn call_gateway_json(&self, service_id: &str, method_name: &str, request: serde_json::Value) -> Result<serde_json::Value, String> {
        let payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let bytes = (self.host.call_service_v1)(RString::from(service_id), MethodName::from(method_name), Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())?;
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| format!("service='{service_id}' method='{method_name}' returned non-json: {e}"))
    }
}

fn extension_of_ref(reference: &str) -> Option<String> {
    let (path, _) = split_asset_ref(reference);
    path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase())
}

fn refs_to_edges(mut refs: Vec<String>, default_role: &str) -> Vec<(String, String, bool)> {
    refs.sort();
    refs.dedup();
    refs.into_iter()
        .map(|reference| {
            let role = match extension_of_ref(&reference).as_deref() {
                Some("ydd") => "drawable_dictionary",
                Some("nemat") => "material_library",
                Some("ytd") => "texture_dictionary",
                Some("ybn") | Some("ycol") => "physics_dictionary",
                Some("nebrain") => "ai_brain",
                Some("nepat") => "ai_pattern",
                Some("nemem") => "ai_memory",
                Some("ytyp") => "definition_ref",
                _ => default_role,
            };
            (reference, role.to_owned(), true)
        })
        .collect()
}

fn definition_entry_refs_to_edges(refs_value: Option<&serde_json::Value>, owner_ref: &str) -> Vec<(String, String, bool)> {
    let Some(refs_value) = refs_value.and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    for (field, role) in [
        ("drawable_refs", "definition/drawable_dependency"),
        ("material_refs", "definition/material_dependency"),
        ("texture_refs", "definition/texture_dependency"),
        ("physics_refs", "definition/physics_dependency"),
        ("collision_refs", "definition/collision_dependency"),
        ("ai_refs", "definition/ai_dependency"),
        ("streaming_refs", "definition/streaming_dependency"),
        ("editor_refs", "definition/editor_dependency"),
        ("other_refs", "definition/other_dependency"),
    ] {
        let Some(items) = refs_value.get(field).and_then(|value| value.as_array()) else {
            continue;
        };
        for item in items {
            let Some(text) = item.as_str() else { continue; };
            let reference = normalize_asset_ref(text);
            if reference.is_empty() || reference == owner_ref {
                continue;
            }
            edges.push((reference, role.to_owned(), true));
        }
    }
    edges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    edges.dedup();
    edges
}

fn collect_ref_strings(value: &serde_json::Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_ref_strings_into(value, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_ref_strings_into(value: &serde_json::Value, refs: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            let normalized = normalize_asset_ref(text);
            if looks_like_runtime_asset_ref(&normalized) {
                refs.push(normalized);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items { collect_ref_strings_into(item, refs); }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() { collect_ref_strings_into(value, refs); }
        }
        _ => {}
    }
}

fn looks_like_runtime_asset_ref(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        ".ytyp@", ".ydd@", ".ydr@", ".yft@", ".nemat@", ".ytd@", ".ymap@",
        ".ymf@", ".ymt@", ".ybn@", ".ybd@", ".ycol@", ".ycd@", ".yed@",
        ".yfd@", ".yld@", ".ypdb@", ".yvr@", ".ywr@", ".ysc@", ".ytf@",
        ".nebrain@", ".nepat@", ".nemem@", ".negoal@", ".nebt@", ".nebehavior@",
        ".neutility@", ".nebb@",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn collect_metadata_namespaces(graph: &mut ResolvedAssetGraphV2, owner_ref: &str, value: &serde_json::Value) {
    if let Some(namespaces) = value.get("metadata_namespaces").or_else(|| value.get("metadata")).and_then(|v| v.as_array()) {
        for namespace in namespaces {
            if let Some(name) = namespace.get("namespace").or_else(|| namespace.get("name")).and_then(|v| v.as_str()) {
                attach_metadata_namespace(graph, owner_ref, name);
            }
        }
    }
    if let Some(side_effects) = value.get("side_effects").and_then(|v| v.as_object()) {
        for key in side_effects.keys() { attach_metadata_namespace(graph, owner_ref, format!("side_effect:{key}")); }
    }
}

fn vfs_source_from_trace(path: &str, trace: &serde_json::Value) -> AssetGraphVfsSource {
    let source = first_object(trace, &["selected", "source", "resolved", "winner", "active_source"]).unwrap_or(trace);
    let source_kind = first_string(source, &["source_kind", "kind", "layer_kind", "type"])
        .unwrap_or_else(|| infer_source_kind(source));
    let physical_path = first_string(source, &["physical_path", "path", "resolved_path", "filesystem_path"]);
    let package_path = first_string(source, &["package_path", "container_path", "nepak", "package"]);
    let package_entry = first_string(source, &["package_entry", "entry", "virtual_path"]);
    let layer_id = first_string(source, &["layer_id", "mount_id", "source_id"]);
    let overridden_by = source
        .get("overridden_by")
        .or_else(|| source.get("shadowed_by"))
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(|v| v.as_str().map(ToOwned::to_owned)).collect())
        .unwrap_or_default();
    AssetGraphVfsSource { source_kind, logical_path: path.to_owned(), physical_path, package_path, package_entry, layer_id, overridden_by }
}

fn first_object<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    for key in keys {
        if let Some(object) = value.get(*key).filter(|v| v.is_object()) { return Some(object); }
    }
    None
}

fn first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
            return Some(text.to_owned());
        }
    }
    None
}

fn infer_source_kind(value: &serde_json::Value) -> String {
    if value.get("package_path").or_else(|| value.get("container_path")).is_some() { return "nepak_package".to_owned(); }
    if value.get("physical_path").or_else(|| value.get("filesystem_path")).is_some() { return "loose_file".to_owned(); }
    "unresolved".to_owned()
}

fn asset_graph_gateway_info() -> serde_json::Value {
    serde_json::json!({
        "service_id": newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        "gateway": newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        "provider": "EngineOwnedAssetGraphResolverProviderV2",
        "contract": "newengine.assets.graph.runtime.v1",
        "methods": newengine_model_domain_api::ASSET_GRAPH_METHODS,
        "schema": newengine_model_domain_api::ASSET_GRAPH_RESOLVED_SCHEMA_V2,
    })
}

fn asset_graph_invoke(state: &mut AssetGraphGatewayState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) { Ok(value) => value, Err(e) => return RResult::RErr(RString::from(e)) };
    let method = value.get("method").and_then(|value| value.as_str()).unwrap_or(newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1);
    let request_value = value.get("request").cloned().unwrap_or_else(|| value.clone());
    let request = serde_json::from_value::<newengine_model_domain_api::AssetGraphResolveRequest>(request_value).unwrap_or_default();
    match method {
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1 => ok_json(state.resolve(request.root())),
        newengine_model_domain_api::ASSET_GRAPH_METHOD_VALIDATE_V1 => {
            let graph = state.resolve(request.root());
            ok_json(newengine_model_domain_api::AssetGraphResolver::validate_graph(graph))
        }
        newengine_model_domain_api::ASSET_GRAPH_METHOD_DUMP_JSON_V1 => match serde_json::to_value(state.resolve(request.root())) {
            Ok(value) => ok_json(value),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        },
        other => RResult::RErr(RString::from(format!("engine.assets.graph: unknown invoke method '{other}'"))),
    }
}

fn asset_graph_service(host: HostApiV1, client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = newengine_service_kit::engine_owned_service_description(
        newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        "newengine-asset-graph-runtime.hydrated-resolver-v2",
        newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHODS.iter().copied(),
    )
    .protocol("newengine.assets.graph.runtime.v1")
    .features(["assets-graph-resolver-v2", "hydrated-dependencies", "vfs-source-trace", "stable-cache-key"])
    .gateway("engine-owned engine.assets.graph resolver")
    .notes("Hydrates dependency graphs through engine.assets.definitions, engine.assets.models, engine.assets.materials, engine.assets.textures and engine.assets/VFS diagnostics.");

    newengine_service_kit::JsonServiceRouter::with_state(
        newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        AssetGraphGatewayState { host, client },
    )
    .describe_json(&description)
    .get_json(newengine_service_api::SERVICE_METHOD_INFO_JSON, |_state| asset_graph_gateway_info())
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, newengine_model_domain_api::ResolvedAssetGraphV2, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        |state, request| Ok(state.resolve(request.root())),
    )
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, newengine_model_domain_api::AssetGraphValidationResult, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_VALIDATE_V1,
        |state, request| {
            let graph = state.resolve(request.root());
            Ok(newengine_model_domain_api::AssetGraphResolver::validate_graph(graph))
        },
    )
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, serde_json::Value, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_DUMP_JSON_V1,
        |state, request| serde_json::to_value(state.resolve(request.root())).map_err(|e| e.to_string()),
    )
    .blob(newengine_service_api::SERVICE_METHOD_INVOKE_JSON, asset_graph_invoke)
    .blob(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| newengine_service_kit::ok_empty_blob())
    .into_service_v1()
}

pub fn register_asset_graph_gateway_best_effort(host: HostApiV1, client: AssetServiceClient) -> bool {
    newengine_service_kit::register_engine_owned_gateway_service_best_effort(
        newengine_service_kit::EngineOwnedGatewayDecl {
            gateway: newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::AssetGraph,
            provider_service: newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
            capability: newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-asset-graph-runtime.hydrated-resolver-v2",
            service: asset_graph_service(host, client),
        },
    )
}
