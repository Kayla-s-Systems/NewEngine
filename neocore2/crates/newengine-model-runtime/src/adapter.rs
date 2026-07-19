//! Model asset adapter and local drawable decoding helpers.
//!
//! This module owns provider-local interpretation of `.ydd`/model payloads and
//! construction DTO assembly. `lib.rs` keeps only the public gateway facade.

use super::*;

#[derive(Clone)]
pub struct ModelAssetAdapter {
    client: AssetServiceClient,
    host: Option<HostApiV1>,
}

struct LoadedModelParts {
    parts: Vec<ModelMeshPart>,
    properties_ref: Option<String>,
    configuration: ModelRuntimeConfiguration,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct DefinitionEntryProjection {
    refs: DefinitionRefsProjection,
    model_explanation: ModelExplanationProjection,
    arbitrary_metadata: serde_json::Value,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct DefinitionRefsProjection {
    drawable_refs: Vec<String>,
    material_refs: Vec<String>,
    texture_refs: Vec<String>,
    uv_layout_refs: Vec<String>,
    physics_refs: Vec<String>,
    collision_refs: Vec<String>,
    ai_refs: Vec<String>,
    streaming_refs: Vec<String>,
    editor_refs: Vec<String>,
    other_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct ModelExplanationProjection {
    model_ref: Option<String>,
    drawable_ref: Option<String>,
    material_bindings: Vec<newengine_model_domain_api::MaterialBindingRef>,
    material_refs: Vec<String>,
    texture_refs: Vec<String>,
    uv_layout_refs: Vec<String>,
    physics_refs: Vec<String>,
    collision_refs: Vec<String>,
    render_options: newengine_model_domain_api::MeshRenderOptions,
    collision_policy: String,
    uv_policy: String,
    physics_policy: String,
    lod_policy: String,
    streaming_policy: String,
}

impl Default for ModelExplanationProjection {
    fn default() -> Self {
        Self {
            model_ref: None,
            drawable_ref: None,
            material_bindings: Vec::new(),
            material_refs: Vec::new(),
            texture_refs: Vec::new(),
            uv_layout_refs: Vec::new(),
            physics_refs: Vec::new(),
            collision_refs: Vec::new(),
            render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
            collision_policy: "unspecified".to_owned(),
            uv_policy: "authored".to_owned(),
            physics_policy: "unspecified".to_owned(),
            lod_policy: "unspecified".to_owned(),
            streaming_policy: "unspecified".to_owned(),
        }
    }
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
        let requested_texture_dictionary = request
            .texture_dictionary
            .as_deref()
            .map(|path| normalize_logical_path(path, false))
            .transpose()?
            .filter(|path| {
                path.ends_with(&format!(".{}", newengine_asset_format_nef8::ytd::EXTENSION))
            });

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
                material_slot: part.material_slot,
                mesh: part.mesh,
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

    fn load_ydd_runtime_parts(
        &self,
        source: &str,
        selector: Option<&str>,
        request_properties_ref: Option<&str>,
    ) -> Result<LoadedModelParts, String> {
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: source.to_owned(),
                output_kind: newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
            })
            .map_err(|e| {
                format!(
                    "engine.assets decode_v1 failed path='{source}' output='{}' err='{e}'",
                    newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT
                )
            })?;
        let document = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&bytes)
            .map_err(|error| {
                format!("model.api: binary .ydd decode failed path='{source}' err='{error}'")
            })?;
        let entry = document.select_entry(selector, true).map_err(|error| {
            format!("model.api: binary .ydd selection failed path='{source}' err='{error}'")
        })?;
        let properties_ref = entry
            .properties_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                request_properties_ref
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });
        let configuration = properties_ref
            .as_deref()
            .map(|reference| self.load_model_configuration(reference))
            .transpose()?
            .unwrap_or_default();
        let descriptor_bindings = configuration
            .material_bindings
            .iter()
            .map(|binding| (binding.slot.clone(), binding.material_ref.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut out = Vec::with_capacity(entry.meshes.len());
        for source_mesh in &entry.meshes {
            let material_slot = source_mesh.material_slot();
            let descriptor_material_ref = descriptor_bindings.get(&material_slot).cloned();
            let material_ref = descriptor_material_ref.or_else(|| source_mesh.material_ref.clone());
            let material = match material_ref
                .as_deref()
                .and_then(|reference| self.load_material_binding_from_ref(reference))
            {
                Some(mut binding) => {
                    binding.slot = material_slot.clone();
                    binding
                }
                None => ModelMaterialBinding {
                    slot: material_slot.clone(),
                    material_ref,
                    fallback_color: [0.82, 0.78, 0.72, 1.0],
                    ..ModelMaterialBinding::default()
                },
            };
            let vertices = source_mesh
                .vertices
                .iter()
                .map(|vertex| PrimitiveVertex {
                    pos: vertex.position,
                    nrm: vertex.normal,
                    uv: vertex.uv0,
                })
                .collect::<Vec<_>>();
            let min = Vec3::new(
                source_mesh.bounds_min[0],
                source_mesh.bounds_min[1],
                source_mesh.bounds_min[2],
            );
            let max = Vec3::new(
                source_mesh.bounds_max[0],
                source_mesh.bounds_max[1],
                source_mesh.bounds_max[2],
            );
            let bounds_center = (min + max) * 0.5;
            let bounds_radius = recompute_bounds_radius(bounds_center, &vertices);
            out.push(ModelMeshPart {
                material_slot,
                mesh: PrimitiveMesh {
                    vertices,
                    indices: source_mesh.indices.clone(),
                    bounds_center,
                    bounds_radius,
                },
                material,
            });
        }
        if out.is_empty() {
            return Err(format!(
                "model.api: binary .ydd selector '{}' produced no mesh parts path='{source}'",
                selector.unwrap_or("<first>")
            ));
        }
        Ok(LoadedModelParts {
            parts: out,
            properties_ref,
            configuration,
        })
    }

    fn load_model_configuration(
        &self,
        properties_ref: &str,
    ) -> Result<ModelRuntimeConfiguration, String> {
        let host = self.host.as_ref().ok_or_else(|| {
            format!(
                "model.api: properties_ref='{properties_ref}' requires engine.assets.definitions host access"
            )
        })?;
        let payload = serde_json::to_vec(&serde_json::json!({
            "definition_ref": properties_ref,
        }))
        .map_err(|e| e.to_string())?;
        let bytes = (host.call_service_v1)(
            RString::from(newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID),
            MethodName::from(newengine_assets_api::definitions_method::ENTRY_JSON_V1),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|err| err.to_string())?;
        let projection = serde_json::from_slice::<DefinitionEntryProjection>(&bytes).map_err(|e| {
            format!(
                "model.api: .ytyp descriptor returned invalid Definition Entry ref='{properties_ref}' err='{e}'"
            )
        })?;
        model_configuration_from_projection(
            normalize_logical_path(properties_ref, true)?,
            projection,
        )
    }

    fn load_nef8_ymt_skeleton_metadata(
        &self,
        source: &str,
        target_height: f32,
        eye_height_ratio: f32,
    ) -> Result<ModelSkeletonMetadata, String> {
        let path = source.split('@').next().unwrap_or(source);
        if !path
            .to_ascii_lowercase()
            .ends_with(&format!(".{}", newengine_asset_format_nef8::ymt::EXTENSION))
        {
            return Err(format!("model skeleton metadata requires provider-declared NEF8 skeleton metadata source, got '{source}'"));
        }
        let bytes = self
            .client
            .decode_v1(&AssetDecodeRequest {
                logical_path: path.to_owned(),
                output_kind: newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
            })
            .map_err(|e| {
                format!(
                    "engine.assets decode_v1 failed path='{path}' output='{}' err='{e}'",
                    newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT
                )
            })?;
        let hash = format!("fnv1a64:{:016x}", fnv1a64(&bytes));
        Ok(ModelSkeletonMetadata {
            source: source.to_owned(),
            source_format: "newengine.ymt.metadata.v1".to_owned(),
            container_magic: "NEF8".to_owned(),
            byte_len: bytes.len(),
            content_hash: hash,
            decode_status: "metadata-only skeleton anchors generated from model target height"
                .to_owned(),
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
        String::from_utf8(bytes)
            .map_err(|e| format!("asset text is not UTF-8 path='{path}' err='{e}'"))
    }
}

fn model_configuration_from_projection(
    properties_ref: String,
    projection: DefinitionEntryProjection,
) -> Result<ModelRuntimeConfiguration, String> {
    let explanation = projection.model_explanation;
    let refs = projection.refs;
    Ok(ModelRuntimeConfiguration {
        properties_ref: Some(properties_ref),
        model_ref: explanation.model_ref,
        drawable_ref: explanation.drawable_ref,
        material_bindings: explanation.material_bindings,
        material_refs: merge_refs(explanation.material_refs, refs.material_refs),
        texture_refs: merge_refs(explanation.texture_refs, refs.texture_refs),
        uv_layout_refs: merge_refs(explanation.uv_layout_refs, refs.uv_layout_refs),
        physics_refs: merge_refs(explanation.physics_refs, refs.physics_refs),
        collision_refs: merge_refs(explanation.collision_refs, refs.collision_refs),
        ai_refs: refs.ai_refs,
        streaming_refs: refs.streaming_refs,
        editor_refs: refs.editor_refs,
        other_refs: refs.other_refs,
        render_options: explanation.render_options,
        collision_policy: explanation.collision_policy,
        uv_policy: explanation.uv_policy,
        physics_policy: explanation.physics_policy,
        lod_policy: explanation.lod_policy,
        streaming_policy: explanation.streaming_policy,
        metadata: projection.arbitrary_metadata,
        warnings: projection.warnings,
    })
}

fn merge_refs(mut primary: Vec<String>, secondary: Vec<String>) -> Vec<String> {
    primary.extend(secondary);
    primary.retain(|reference| !reference.trim().is_empty());
    primary.sort();
    primary.dedup();
    primary
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

fn default_humanoid_joints(
    target_height: f32,
) -> Vec<newengine_model_skeleton_api::ModelSkeletonJointMetadata> {
    use newengine_model_skeleton_api::skeleton_joint;
    vec![
        skeleton_joint("root", Option::<String>::None, [0.0, 0.0, 0.0]),
        skeleton_joint("hips", Some("root"), [0.0, target_height * 0.50, 0.0]),
        skeleton_joint("spine", Some("hips"), [0.0, target_height * 0.68, 0.0]),
        skeleton_joint("head", Some("spine"), [0.0, target_height * 0.91, 0.0]),
        skeleton_joint(
            "left_hand",
            Some("spine"),
            [-0.42, target_height * 0.58, 0.0],
        ),
        skeleton_joint(
            "right_hand",
            Some("spine"),
            [0.42, target_height * 0.58, 0.0],
        ),
        skeleton_joint("left_foot", Some("hips"), [-0.16, 0.02, 0.0]),
        skeleton_joint("right_foot", Some("hips"), [0.16, 0.02, 0.0]),
        skeleton_joint("eye", Some("head"), [0.0, target_height * 0.91, -0.08]),
    ]
}

fn default_humanoid_anchors(
    target_height: f32,
    eye_height_ratio: f32,
) -> newengine_model_skeleton_api::ModelSkeletonAnchors {
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
