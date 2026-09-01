//! Model asset adapter and local drawable decoding helpers.
//!
//! This module owns provider-local interpretation of `.ydd`/model payloads and
//! construction DTO assembly. `lib.rs` keeps only the public gateway facade.

use super::*;

mod fallback_humanoid;
mod projection;
mod runtime_parts;

use projection::{model_configuration_from_projection, DefinitionEntryProjection};

#[derive(Clone)]
pub struct ModelAssetAdapter {
    client: AssetServiceClient,
    host: Option<HostApiV1>,
}

impl ModelAssetAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self { client, host: None }
    }

    #[inline]
    pub fn with_client_and_host(client: AssetServiceClient, host: HostApiV1) -> Self {
        Self {
            client,
            host: Some(host),
        }
    }

    /// Enqueue an opaque SpeedTree SRT/SPM source through AssetManager.
    ///
    /// No SDK/parser code lives here: the active AssetImporterV1 provider owns
    /// source decoding, cache keys and compiled runtime outputs.
    pub fn import_foliage_source(
        &self,
        request: &FoliageImportRequestV1,
    ) -> Result<FoliageImportResponseV1, String> {
        let settings = request.settings.clone().sanitized()?;
        let runtime_asset_ref = settings.runtime_asset_ref()?;
        let importer_id = settings.importer_id()?.to_owned();
        let asset_id = self.client.import_v1(&settings.canonical_path)?;
        Ok(FoliageImportResponseV1 {
            accepted: true,
            canonical_source_ref: settings.canonical_path,
            runtime_asset_ref,
            importer_id,
            asset_id,
            queue_status: "accepted".to_owned(),
            ..FoliageImportResponseV1::default()
        })
    }

    pub fn load_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let request = self.resolve_request(request)?;
        let target_height = request.target_height.clamp(0.25, 3.0);
        let request_properties_ref = request
            .properties_ref
            .as_deref()
            .map(|path| normalize_logical_path(path, true))
            .transpose()?;
        let source_ref = normalize_logical_path(&request.model, true)?;
        let (source_path, selector) = split_model_selector(&source_ref);
        let requested_texture_dictionary = match request.texture_dictionary.as_deref() {
            Some(path) => {
                let path = normalize_logical_path(path, false)?;
                self.client
                    .require_semantic_asset_reference_v1(
                        &path,
                        newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID,
                        false,
                    )
                    .map_err(|error| {
                        format!(
                            "model.api: texture_dictionary must resolve through the registered texture asset format path='{path}': {error}"
                        )
                    })?;
                Some(path)
            }
            None => None,
        };

        if has_extension(&source_path, DRAWABLE_DICTIONARY_EXTENSION) {
            let loaded = self.load_ydd_runtime_parts(
                &source_path,
                selector.as_deref(),
                request_properties_ref.as_deref(),
            )?;
            let properties_ref = loaded.properties_ref.or(request_properties_ref);
            let configuration = loaded.configuration;
            let dependency_graph = request.dependency_graph.clone().unwrap_or_default();
            let texture_dictionary = requested_texture_dictionary
                .or_else(|| first_texture_dictionary(&configuration.texture_refs));
            let skeleton = match request.skeleton.as_deref() {
                Some(path) => Some(self.load_skeleton_metadata(
                    path,
                    target_height,
                    request.eye_height_ratio,
                )?),
                None => None,
            };
            let collisions = if request.collisions.is_empty() {
                newengine_model_collision_runtime::default_collisions_for_model(
                    skeleton.as_ref(),
                    target_height,
                )
            } else {
                request.collisions.clone()
            };
            return Ok(ModelAssetBundle {
                source: source_ref,
                properties_ref,
                parts: loaded.parts,
                skeleton,
                texture_dictionary,
                collisions,
                configuration,
                dependency_graph,
            });
        }

        let source = normalize_logical_path(&request.model, false)?;
        let configuration = request_properties_ref
            .as_deref()
            .map(|reference| self.load_model_configuration(reference))
            .transpose()?
            .unwrap_or_default();
        let dependency_graph = request.dependency_graph.clone().unwrap_or_default();
        let texture_dictionary = requested_texture_dictionary
            .or_else(|| first_texture_dictionary(&configuration.texture_refs));
        let skeleton = match request.skeleton.as_deref() {
            Some(path) => {
                Some(self.load_skeleton_metadata(path, target_height, request.eye_height_ratio)?)
            }
            None => None,
        };

        let obj_text = self.read_text(&source)?;
        let decoded = newengine_model_import_obj::decode_obj_with_mtl_loader(
            &source,
            &obj_text,
            target_height,
            |path| self.read_text(path).ok(),
        )?;

        let descriptor_bindings = configuration
            .material_bindings
            .iter()
            .map(|binding| (binding.slot.clone(), binding.material_ref.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut parts = Vec::with_capacity(decoded.parts.len());
        for part in decoded.parts {
            let descriptor_material_ref = descriptor_bindings.get(&part.material_slot).cloned();
            let material = match descriptor_material_ref
                .as_deref()
                .and_then(|material_ref| self.load_material_binding_from_ref(material_ref))
            {
                Some(mut binding) => {
                    binding.slot = part.material_slot.clone();
                    binding
                }
                None => newengine_material_runtime::material_binding(
                    &part.material_slot,
                    decoded.materials.get(&part.material_slot),
                    texture_dictionary.as_deref(),
                ),
            };
            parts.push(ModelMeshPart {
                source_mesh_name: String::new(),
                material_slot: part.material_slot,
                mesh: part.mesh,
                skin: None,
                material,
            });
        }

        let collisions = if request.collisions.is_empty() {
            newengine_model_collision_runtime::default_collisions_for_model(
                skeleton.as_ref(),
                target_height,
            )
        } else {
            request.collisions.clone()
        };

        Ok(ModelAssetBundle {
            source,
            properties_ref: request_properties_ref,
            parts,
            skeleton,
            texture_dictionary,
            collisions,
            configuration,
            dependency_graph,
        })
    }

    pub fn load_manifest(&self, logical_path: &str) -> Result<ModelConstructionManifest, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let text = self.read_text(&source)?;
        serde_json::from_str::<ModelConstructionManifest>(&text)
            .map_err(|e| format!("model manifest parse failed path='{source}' err='{e}'"))
    }

    pub fn resolve_request(
        &self,
        request: &ModelAssetRequest,
    ) -> Result<ModelAssetRequest, String> {
        let mut resolved = request.clone();
        if let Some(manifest_path) = request.manifest.as_deref() {
            let manifest = self.load_manifest(manifest_path)?;
            if resolved.model.trim().is_empty() {
                resolved.model = manifest.model;
            }
            if resolved.skeleton.is_none() {
                resolved.skeleton = manifest.skeleton.map(|it| it.source);
            }
            if resolved.properties_ref.is_none() {
                resolved.properties_ref = manifest.properties_ref;
            }
            if resolved.texture_dictionary.is_none() {
                resolved.texture_dictionary = manifest.material_set.texture_dictionary;
            }
            if resolved.collisions.is_empty() {
                resolved.collisions = manifest.collisions;
            }
            if (resolved.target_height - ModelAssetRequest::default().target_height).abs()
                < f32::EPSILON
            {
                resolved.target_height = manifest.target_height;
            }
            if (resolved.eye_height_ratio - ModelAssetRequest::default().eye_height_ratio).abs()
                < f32::EPSILON
            {
                resolved.eye_height_ratio = manifest.eye_height_ratio;
            }
        }

        if resolved.model.trim().is_empty() {
            let properties_ref = resolved.properties_ref.as_deref().ok_or_else(|| {
                "model request requires model, manifest, or properties_ref".to_owned()
            })?;
            let configuration = self.load_model_configuration(properties_ref)?;
            resolved.model = configuration
                .drawable_ref
                .clone()
                .or(configuration.model_ref.clone())
                .ok_or_else(|| {
                    format!(
                        "model properties '{}' declares neither drawable_ref nor model_ref",
                        properties_ref
                    )
                })?;
            if resolved.texture_dictionary.is_none() {
                resolved.texture_dictionary = first_texture_dictionary(&configuration.texture_refs);
            }
        }
        Ok(resolved)
    }

    pub fn validate_request(&self, request: &ModelAssetRequest) -> ModelConstructionValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let resolved = match self.resolve_request(request) {
            Ok(resolved) => Some(resolved),
            Err(e) => {
                errors.push(e);
                None
            }
        };
        if let Some(resolved) = resolved.as_ref() {
            if resolved.model.trim().is_empty() {
                errors.push("model asset path is empty after manifest resolution".to_owned());
            }
            if resolved.skeleton.is_none() {
                warnings.push(
                    "no skeleton source declared; runtime model will use mesh-only binding"
                        .to_owned(),
                );
            }
            if resolved.properties_ref.is_none() {
                warnings.push("no .ytyp properties_ref declared; .ydd entry must provide descriptor for material binding".to_owned());
            }
            if resolved.texture_dictionary.is_none() {
                warnings.push("no texture dictionary declared; graph/material resolution should provide .ytd refs".to_owned());
            }
        }
        ModelConstructionValidation {
            valid: errors.is_empty(),
            resolved,
            errors,
            warnings,
        }
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
            return Err(format!(
                "{label} requires .{} source, got '{source}'",
                extension.trim_start_matches('.')
            ));
        }
        let bytes = self.client.decode_v1(&AssetDecodeRequest {
            logical_path: source.clone(),
            output_kind: output_kind.to_owned(),
            selector: selector
                .map(|selector| serde_json::json!({ "selector": selector, "entry": selector }))
                .unwrap_or(serde_json::Value::Null),
                    format_descriptor: None,
})
        .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' output='{output_kind}' err='{e}'"))?;
        serde_json::from_slice::<T>(&bytes).map_err(|e| {
            format!("model.api: {label} decode returned invalid json path='{source}' err='{e}'")
        })
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

    fn read_bytes(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        let path = normalize_logical_path(logical_path, false)?;
        self.client
            .raw_bytes_v1(&path)
            .map_err(|e| format!("asset.raw_bytes_v1 failed path='{path}' err='{e}'"))
    }

    fn read_text(&self, logical_path: &str) -> Result<String, String> {
        let path = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&path)?;
        String::from_utf8(bytes)
            .map_err(|e| format!("asset text is not UTF-8 path='{path}' err='{e}'"))
    }
}

fn first_texture_dictionary(texture_refs: &[String]) -> Option<String> {
    texture_refs.iter().find_map(|reference| {
        let dictionary = reference.split('@').next().unwrap_or(reference).trim();
        (!dictionary.is_empty()).then(|| dictionary.to_owned())
    })
}

fn split_model_selector(source: &str) -> (String, Option<String>) {
    match source.rsplit_once('@') {
        Some((path, selector)) => (
            path.to_owned(),
            Some(selector.to_owned()).filter(|it| !it.trim().is_empty()),
        ),
        None => (source.to_owned(), None),
    }
}

impl ModelAssetAdapter {
    fn load_material_binding_from_ref(&self, material_ref: &str) -> Option<ModelMaterialBinding> {
        let host = self.host.as_ref()?;
        let request = newengine_materials::MaterialLoadRequest {
            logical_path: material_ref.to_owned(),
            selector: None,
        };
        let payload = serde_json::to_vec(&request).ok()?;
        let bytes = (host.call_service_v1)(
            RString::from(newengine_materials::ENGINE_ASSETS_MATERIALS_SERVICE_ID),
            MethodName::from(newengine_materials::method::LOAD_DESCRIPTOR_V1),
            Blob::from(payload),
        )
        .into_result()
        .ok()?
        .into_vec();
        let response: newengine_materials::MaterialDescriptorLoadResponse =
            serde_json::from_slice(&bytes).ok()?;
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

fn has_extension(path: &str, extension: &str) -> bool {
    let expected = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    path.split('@')
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
        .ends_with(&format!(".{expected}"))
}

#[cfg(test)]
mod runtime_configuration_tests {
    use super::*;
    use projection::{DefinitionRefsProjection, ModelExplanationProjection};

    #[test]
    fn ytyp_projection_preserves_full_runtime_dependency_configuration() {
        let projection = DefinitionEntryProjection {
            refs: DefinitionRefsProjection {
                material_refs: vec!["materials/car.nemat@paint".to_owned()],
                texture_refs: vec!["textures/car.ytd@paint_bc".to_owned()],
                uv_layout_refs: vec!["models/car.ytyd@body".to_owned()],
                physics_refs: vec!["physics/car.ybn@body".to_owned()],
                streaming_refs: vec!["stream/car.ymf@main".to_owned()],
                ..Default::default()
            },
            model_explanation: ModelExplanationProjection {
                drawable_ref: Some("models/car.ydd@body".to_owned()),
                material_bindings: vec![newengine_model_domain_api::MaterialBindingRef {
                    slot: "paint".to_owned(),
                    material_ref: "materials/car.nemat@paint".to_owned(),
                    required: true,
                }],
                render_options: newengine_model_domain_api::MeshRenderOptions::world_masked(),
                lod_policy: "authored_lods".to_owned(),
                streaming_policy: "distance_streamed".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let config =
            model_configuration_from_projection("definitions/car.ytyp".to_owned(), projection)
                .unwrap();
        assert_eq!(config.drawable_ref.as_deref(), Some("models/car.ydd@body"));
        assert_eq!(config.material_bindings.len(), 1);
        assert_eq!(config.texture_refs, vec!["textures/car.ytd@paint_bc"]);
        assert_eq!(config.uv_layout_refs, vec!["models/car.ytyd@body"]);
        assert_eq!(config.physics_refs, vec!["physics/car.ybn@body"]);
        assert_eq!(config.streaming_refs, vec!["stream/car.ymf@main"]);
        assert_eq!(config.lod_policy, "authored_lods");
        assert_eq!(config.streaming_policy, "distance_streamed");
        assert_eq!(
            config.render_options,
            newengine_model_domain_api::MeshRenderOptions::world_masked()
        );
    }
}
