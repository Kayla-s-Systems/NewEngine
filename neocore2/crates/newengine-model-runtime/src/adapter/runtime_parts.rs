use super::fallback_humanoid::{default_humanoid_anchors, default_humanoid_joints};
use super::*;

pub(super) struct LoadedModelParts {
    pub(super) parts: Vec<ModelMeshPart>,
    pub(super) properties_ref: Option<String>,
    pub(super) configuration: ModelRuntimeConfiguration,
}

impl ModelAssetAdapter {
    pub(super) fn load_ydd_runtime_parts(
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
            let skin = source_mesh
                .skin
                .as_ref()
                .map(|stream| {
                    let source_to_model = entry.skin_source_to_model.ok_or_else(|| {
                        format!(
                            "model.api: skinned binary .ydd entry '{}' has no source-to-model transform path='{}'",
                            entry.name, source
                        )
                    })?;
                    Ok::<_, String>(ModelSkinBinding {
                        vertices: stream
                            .iter()
                            .map(|vertex| ModelSkinVertex {
                                joints: vertex.joints,
                                weights: vertex.weights,
                                joints_extra: vertex.joints_extra,
                                weights_extra: vertex.weights_extra,
                            })
                            .collect(),
                        source_to_model,
                    })
                })
                .transpose()?;
            out.push(ModelMeshPart {
                material_slot,
                mesh: PrimitiveMesh {
                    vertices,
                    indices: source_mesh.indices.clone(),
                    bounds_center,
                    bounds_radius,
                },
                skin,
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

    pub(super) fn load_model_configuration(
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

    pub(super) fn load_nef8_ymt_skeleton_metadata(
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
        if let Some(decoded) =
            crate::skeleton_metadata::decode_skeleton_body(&bytes, target_height, eye_height_ratio)?
        {
            return Ok(ModelSkeletonMetadata {
                source: source.to_owned(),
                source_format: decoded.source_format,
                container_magic: "NEF8".to_owned(),
                byte_len: bytes.len(),
                content_hash: hash,
                decode_status: decoded.decode_status,
                joints: decoded.joints,
                anchors: decoded.anchors,
            });
        }

        Ok(ModelSkeletonMetadata {
            source: source.to_owned(),
            source_format: "newengine.ymt.metadata.v1".to_owned(),
            container_magic: "NEF8".to_owned(),
            byte_len: bytes.len(),
            content_hash: hash,
            decode_status:
                "legacy metadata-only skeleton; generated humanoid anchors from model target height"
                    .to_owned(),
            joints: default_humanoid_joints(target_height),
            anchors: default_humanoid_anchors(target_height, eye_height_ratio),
        })
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
