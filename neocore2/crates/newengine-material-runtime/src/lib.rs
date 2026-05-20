#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material mapping for imported model material sources.

use newengine_materials::{MaterialDescriptor, MaterialFlags, MaterialTextureBindings};
use newengine_model_domain_api::ModelMaterialBinding;
use newengine_model_import_obj::{normalize_logical_path, ModelMaterialSource};

pub fn material_binding(
    material_slot: &str,
    parsed: Option<&ModelMaterialSource>,
    texture_dictionary: Option<&str>,
) -> ModelMaterialBinding {
    let mut color = parsed
        .map(|mat| {
            let authored_white = mat.kd.iter().all(|v| *v >= 0.92);
            if authored_white && mat.base_color_texture.is_some() {
                fallback_slot_color(material_slot)
            } else {
                [mat.kd[0], mat.kd[1], mat.kd[2], mat.alpha]
            }
        })
        .unwrap_or_else(|| fallback_slot_color(material_slot));
    for c in &mut color {
        *c = c.clamp(0.0, 1.0);
    }

    let roughness = parsed
        .map(|mat| (1.0 - (mat.ns / 512.0).clamp(0.0, 0.9)).clamp(0.28, 0.92))
        .unwrap_or(0.78);
    let alpha_flag = if color[3] < 0.99 { MaterialFlags::ALPHA_BLEND } else { MaterialFlags::NONE };
    let flags = MaterialFlags::DOUBLE_SIDED
        .union(MaterialFlags::CAST_SHADOWS)
        .union(MaterialFlags::RECEIVE_SHADOWS)
        .union(alpha_flag);

    let descriptor = MaterialDescriptor { base_color: color, roughness, flags, ..MaterialDescriptor::default() };
    let mut textures = MaterialTextureBindings::default();
    if let Some(texture) = parsed
        .and_then(|mat| mat.base_color_texture.as_deref())
        .and_then(|path| runtime_texture_ref(path, texture_dictionary))
    {
        textures.base_color_texture = Some(texture);
    }
    if let Some(texture) = parsed
        .and_then(|mat| mat.normal_texture.as_deref())
        .and_then(|path| runtime_texture_ref(path, texture_dictionary))
    {
        textures.normal_texture = Some(texture);
    }

    ModelMaterialBinding {
        slot: material_slot.to_owned(),
        descriptor,
        textures: textures.sanitized(),
        fallback_color: color,
    }
}

pub fn runtime_texture_ref(path: &str, texture_dictionary: Option<&str>) -> Option<String> {
    let normalized = normalize_logical_path(path, true).ok()?;
    if normalized.contains(".neytd@") {
        return Some(normalized);
    }

    let (_, file) = normalized.rsplit_once('/').unwrap_or(("", normalized.as_str()));
    let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file).trim();
    if stem.is_empty() {
        return None;
    }

    if let Some(dictionary) = texture_dictionary {
        return Some(format!("{}@{}", dictionary, stem));
    }

    let (base, _) = normalized.rsplit_once('/').unwrap_or(("", normalized.as_str()));
    let fallback_dict = if base.is_empty() { "textures.neytd".to_owned() } else { format!("{}/textures.neytd", base) };
    Some(format!("{}@{}", fallback_dict, stem))
}

pub fn fallback_slot_color(material_slot: &str) -> [f32; 4] {
    let slot = material_slot.to_ascii_lowercase();
    if slot.contains("hair") {
        [0.16, 0.10, 0.08, 1.0]
    } else if slot.contains("skin") || slot.contains("head") || slot.contains("hand") {
        [0.76, 0.58, 0.48, 1.0]
    } else if slot.contains("lowr") {
        [0.16, 0.15, 0.14, 1.0]
    } else if slot.contains("uppr") {
        [0.42, 0.30, 0.23, 1.0]
    } else {
        [0.70, 0.66, 0.60, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_dictionary_selector_is_derived() {
        let selector = runtime_texture_ref("player/abigail/textures/hair_diff_000_a_uni.dds", Some("player/abigail/textures/abigail.neytd"));
        assert_eq!(selector.as_deref(), Some("player/abigail/textures/abigail.neytd@hair_diff_000_a_uni"));
    }
}

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_materials::{
    binary, method as material_method, MaterialLoadRequest,
    MaterialLoadResponse, MaterialTextureRefInfo, MaterialTextureRefRequest,
    ENGINE_MATERIALS_SERVICE_ID, MATERIALS_BACKEND_CAPABILITY_ID, MATERIALS_SERVICE_ID,
    MATERIALS_SERVICE_METHODS,
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
        let source = normalize_material_logical_path(&request.logical_path)?;
        if !source.to_ascii_lowercase().ends_with(".nemat") {
            return Err(format!("materials: expected .nemat material path, got '{source}'"));
        }
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: source.clone(),
                output_kind: "material.raw".to_owned(),
                selector: serde_json::Value::Null,
            })
            .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' err='{e}'"))?;
        let asset = binary::decode_asset(&bytes)
            .map_err(|e| format!("materials: decode .nemat failed path='{source}' err='{e}'"))?;
        let mut descriptor = asset.desc;
        descriptor.sanitize_in_place();
        Ok(MaterialLoadResponse {
            source,
            id: material_id_from_name(&asset.name),
            name: asset.name,
            descriptor,
            textures: MaterialTextureBindings::default(),
        })
    }

    #[inline]
    pub fn describe_texture_ref(&self, request: &MaterialTextureRefRequest) -> MaterialTextureRefInfo {
        MaterialTextureRefInfo::from_reference(&request.reference)
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
                    "asset_kind": "material",
                    "container": "newengine.material.v1",
                    "read_method": material_method::LOAD_JSON_V1,
                    "runtime_ready": true,
                    "notes": "Native NewEngine material descriptor container."
                }
            ],
            "texture_reference_policy": "runtime material textures must be texture dictionary selectors handled by registered codecs"
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
            material_method::LOAD_JSON_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("materials.api: invalid load request: {e}"))),
                };
                match self.adapter.load_material(&request) {
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
            material_method::FORMATS_JSON_V1 => ok_json(self.formats_json()),
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
        texture_reference_policy: ".neytd dictionary selectors only",
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
    .features(["nemat", "neytd-texture-selectors"])
    .notes("Engine material gateway. Native material containers are read through engine.assets/VFS.");

    JsonServiceRouter::with_state(
        MATERIALS_SERVICE_ID,
        MaterialGatewayState::new(MaterialAssetGatewayAdapter::with_client(client)),
    )
    .describe_json(&description)
    .info(materials_service_info)
    .get_json(material_method::FORMATS_JSON_V1, |state| state.formats_json())
    .post_json_result::<MaterialLoadRequest, MaterialLoadResponse, _>(
        material_method::LOAD_JSON_V1,
        |state, request| state.adapter.load_material(&request),
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
