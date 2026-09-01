use std::sync::{Arc, Mutex};

use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_assets_api::{
    textures_method, ASSET_LIST_FILE_BODY_OUTPUT, ENGINE_ASSETS_TEXTURES_SERVICE_ID,
};
use newengine_materials::{
    MaterialDescriptorLoadResponse, MaterialLoadRequest, MaterialLoadResponse,
    MaterialTextureRefInfo, MaterialTextureRefRequest, MaterialValidationRequest,
    MaterialValidationResult, RenderMaterialPacket, ResolvedMaterialGraph,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

use crate::{
    cache::MaterialRuntimeCaches, collect_texture_refs, decode_material_entry_payload,
    material_cache_key, material_response_from_authored, normalize_material_logical_path,
    preview_material_name_from_body, split_nemat_selector, validate_material_body_schema,
};

#[derive(Clone)]
pub struct MaterialAssetGatewayAdapter {
    client: AssetServiceClient,
    host: Option<HostApiV1>,
    caches: Arc<Mutex<MaterialRuntimeCaches>>,
}

impl MaterialAssetGatewayAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self {
            client,
            host: None,
            caches: Arc::new(Mutex::new(MaterialRuntimeCaches::default())),
        }
    }

    #[inline]
    pub fn with_client_and_host(client: AssetServiceClient, host: HostApiV1) -> Self {
        Self {
            client,
            host: Some(host),
            caches: Arc::new(Mutex::new(MaterialRuntimeCaches::default())),
        }
    }

    pub fn preview_material_ref(&self, logical_path: &str) -> Result<String, String> {
        let source = normalize_material_logical_path(
            logical_path.split('@').next().unwrap_or(logical_path),
        )?;
        let (_, descriptor) = self.client
            .require_semantic_asset_reference_v1(
                &source,
                newengine_assets_api::ENGINE_ASSETS_MATERIALS_SERVICE_ID,
                false,
            )
            .map_err(|error| {
                format!(
                    "materials: source must resolve through the registered material format: '{source}': {error}"
                )
            })?;
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: source.clone(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
                            format_descriptor: None,
})
            .map_err(|e| format!(
                "engine.assets decode_v1 failed path='{source}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{e}'"
            ))?;
        validate_material_body_schema(&bytes, &descriptor)?;
        let selector = preview_material_name_from_body(&bytes)?;
        Ok(format!("{source}@{selector}"))
    }

    pub fn load_material(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<MaterialLoadResponse, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        let (_, descriptor) = self.client
            .require_semantic_asset_reference_v1(
                &source,
                newengine_assets_api::ENGINE_ASSETS_MATERIALS_SERVICE_ID,
                false,
            )
            .map_err(|error| {
                format!(
                    "materials: source must resolve through the registered material format: '{source}': {error}"
                )
            })?;
        newengine_ulog_api::ulog::debug!(
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
                            format_descriptor: None,
})
            .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' selector='{selector}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{e}'"))?;
        validate_material_body_schema(&bytes, &descriptor)?;
        let material = decode_material_entry_payload(&bytes, &selector)
            .map_err(|e| format!("materials: decode .nemat library failed source='{source}' selector='{selector}' err='{e}'"))?;
        newengine_ulog_api::ulog::debug!(
            "assets.materials.load_descriptor_v1: decoded source='{}' selector='{}' texture_slots={} params={}",
            source,
            selector,
            material.textures.len(),
            material.params.len()
        );
        material_response_from_authored(&source, &selector, material)
    }

    #[inline]
    pub fn describe_texture_ref(
        &self,
        request: &MaterialTextureRefRequest,
    ) -> MaterialTextureRefInfo {
        self.validate_texture_ref_through_textures_gateway(&request.reference)
    }

    fn validate_texture_ref_through_textures_gateway(
        &self,
        reference: &str,
    ) -> MaterialTextureRefInfo {
        let mut info = MaterialTextureRefInfo::from_reference(reference);
        if !info.valid {
            return info;
        }

        if let Ok(mut caches) = self.caches.lock() {
            if let Some(cached) = caches.texture_refs.get(&info.canonical).cloned() {
                return cached;
            }
        }

        let request = serde_json::json!({ "texture_ref": info.canonical });
        let payload = match serde_json::to_vec(&request) {
            Ok(payload) => payload,
            Err(e) => {
                info.valid = false;
                info.errors.push(format!(
                    "engine.assets.textures validation payload encode failed: {e}"
                ));
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
                info.errors.push(format!(
                    "engine.assets.textures validation unavailable for '{}': {}",
                    info.canonical, e
                ));
                return info;
            }
        };
        let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => value,
            Err(e) => {
                info.valid = false;
                info.errors.push(format!(
                    "engine.assets.textures validation returned non-json for '{}': {}",
                    info.canonical, e
                ));
                return info;
            }
        };
        if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            info.warnings
                .push("validated_by=engine.assets.textures".to_owned());
            if let Ok(mut caches) = self.caches.lock() {
                caches
                    .texture_refs
                    .insert(info.canonical.clone(), info.clone());
            }
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

    pub fn load_descriptor(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<MaterialDescriptorLoadResponse, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        let cache_key = material_cache_key(&source, &selector);
        if let Ok(mut caches) = self.caches.lock() {
            if let Some(cached) = caches.descriptors.get(&cache_key).cloned() {
                newengine_ulog_api::ulog::debug!(
                    "assets.materials.load_descriptor_v1: cache hit source='{}' selector='{}' policy='decoded .nemat entry cache'",
                    source,
                    selector
                );
                return Ok(cached);
            }
        }

        let loaded = self.load_material(&MaterialLoadRequest {
            logical_path: format!("{source}@{selector}"),
            selector: None,
        })?;
        let response = MaterialDescriptorLoadResponse {
            source: loaded.source,
            name: loaded.name,
            shader: loaded.shader,
            descriptor: loaded.descriptor,
            textures: loaded.textures,
            params: loaded.params,
        };
        if let Ok(mut caches) = self.caches.lock() {
            caches.descriptors.insert(cache_key, response.clone());
        }
        Ok(response)
    }

    pub fn validate_material(
        &self,
        request: &MaterialValidationRequest,
    ) -> MaterialValidationResult {
        let mut result = MaterialValidationResult {
            source: request.logical_path.clone(),
            ..Default::default()
        };
        let loaded = match self.load_descriptor(&MaterialLoadRequest {
            logical_path: request.logical_path.clone(),
            selector: request.selector.clone(),
        }) {
            Ok(value) => value,
            Err(err) => {
                result.errors.push(err);
                return result;
            }
        };
        result.source = loaded.source.clone();
        for texture in collect_texture_refs(&loaded.textures) {
            let info = self.validate_texture_ref_through_textures_gateway(texture);
            if !info.valid {
                result.errors.extend(info.errors);
            }
        }
        result.valid = result.errors.is_empty();
        result
    }

    pub fn resolve_graph(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<ResolvedMaterialGraph, String> {
        let material_ref = normalize_material_logical_path(&request.logical_path)?;
        let (source, selector) = split_nemat_selector(&material_ref, request.selector.as_deref())?;
        let cache_key = material_cache_key(&source, &selector);
        if let Ok(mut caches) = self.caches.lock() {
            if let Some(cached) = caches.graphs.get(&cache_key).cloned() {
                newengine_ulog_api::ulog::debug!(
                    "assets.materials.resolve_graph_v1: cache hit source='{}' selector='{}' texture_refs={} warnings={}",
                    source,
                    selector,
                    cached.texture_refs.len(),
                    cached.warnings.len()
                );
                return Ok(cached);
            }
        }

        let loaded = self.load_descriptor(&MaterialLoadRequest {
            logical_path: format!("{source}@{selector}"),
            selector: None,
        })?;
        let mut graph = ResolvedMaterialGraph {
            source: loaded.source,
            name: loaded.name,
            shader: loaded.shader,
            descriptor: loaded.descriptor,
            textures: loaded.textures,
            params: loaded.params,
            ..Default::default()
        };
        for texture in collect_texture_refs(&graph.textures) {
            let info = self.validate_texture_ref_through_textures_gateway(texture);
            if !info.valid {
                graph.warnings.extend(info.errors.clone());
            }
            graph.texture_refs.push(info);
        }
        newengine_ulog_api::ulog::debug!(
            "assets.materials.resolve_graph_v1: source='{}' texture_refs={} warnings={} cache='store'",
            graph.source,
            graph.texture_refs.len(),
            graph.warnings.len()
        );
        if let Ok(mut caches) = self.caches.lock() {
            caches.graphs.insert(cache_key, graph.clone());
        }
        Ok(graph)
    }

    pub fn to_render_packet(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<RenderMaterialPacket, String> {
        let graph = self.resolve_graph(request)?;
        if graph.texture_refs.iter().any(|r| !r.valid) {
            return Err(format!("materials: cannot produce RenderMaterialPacket for '{}' because texture references are invalid", graph.source));
        }
        newengine_ulog_api::ulog::debug!(
            "assets.materials.to_render_packet_v1: source='{}' name='{}' packet_kind='renderer_agnostic_material_packet'",
            graph.source,
            graph.name
        );
        Ok(RenderMaterialPacket {
            source: graph.source,
            name: graph.name,
            shader: graph.shader,
            descriptor: graph.descriptor,
            textures: graph.textures,
            params: graph.params,
            ..Default::default()
        })
    }
}
