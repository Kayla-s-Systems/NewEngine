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
        let properties_ref = request
            .properties_ref
            .as_deref()
            .map(|path| normalize_logical_path(path, true))
            .transpose()?;
        let source_ref = normalize_logical_path(&request.model, true)?;
        let (source_path, selector) = split_model_selector(&source_ref);
        let texture_dictionary = request
            .texture_dictionary
            .as_deref()
            .map(|path| normalize_logical_path(path, false))
            .transpose()?
            .filter(|path| {
                path.ends_with(&format!(".{}", newengine_asset_format_nef8::ytd::EXTENSION))
            });

        let skeleton = match request.skeleton.as_deref() {
            Some(path) => {
                Some(self.load_skeleton_metadata(path, target_height, request.eye_height_ratio)?)
            }
            None => None,
        };

        if has_extension(&source_path, DRAWABLE_DICTIONARY_EXTENSION) {
            let parts = self.load_ydd_runtime_parts(
                &source_path,
                selector.as_deref(),
                properties_ref.as_deref(),
            )?;
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
                parts,
                skeleton,
                texture_dictionary,
                collisions,
            });
        }

        let source = normalize_logical_path(&request.model, false)?;
        let obj_text = self.read_text(&source)?;
        let decoded = newengine_model_import_obj::decode_obj_with_mtl_loader(
            &source,
            &obj_text,
            target_height,
            |path| self.read_text(path).ok(),
        )?;

        let descriptor_bindings = properties_ref.as_deref().and_then(|properties_ref| {
            self.load_material_bindings_from_properties_ref(properties_ref)
                .ok()
        });
        let mut parts = Vec::with_capacity(decoded.parts.len());
        for part in decoded.parts {
            let descriptor_material_ref = descriptor_bindings
                .as_ref()
                .and_then(|bindings| bindings.get(&part.material_slot).cloned());
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
            properties_ref,
            parts,
            skeleton,
            texture_dictionary,
            collisions,
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
        let Some(manifest_path) = request.manifest.as_deref() else {
            return Ok(request.clone());
        };
        let manifest = self.load_manifest(manifest_path)?;
        let mut resolved = request.clone();
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
    ) -> Result<Vec<ModelMeshPart>, String> {
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
        let root: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            format!("model.api: .ydd NEF8 body returned invalid json path='{source}' err='{e}'")
        })?;
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
            .ok_or_else(|| {
                format!("model.api: .ydd has no runtime_mesh_parts array path='{source}'")
            })?;
        let properties_by_entry = ydd_properties_ref_by_entry(&root);
        let root_properties_ref = ydd_root_properties_ref(&root);
        let request_properties_ref = request_properties_ref
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut material_bindings_by_properties =
            std::collections::BTreeMap::<String, std::collections::BTreeMap<String, String>>::new();
        let mut out = Vec::new();
        let single_part_selector_fallback = selector.is_some() && parts.len() == 1;
        for part in parts {
            let entry = part
                .get("entry")
                .or_else(|| part.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !single_part_selector_fallback
                && selector
                    .map(|needle| !needle.eq_ignore_ascii_case(entry))
                    .unwrap_or(false)
            {
                continue;
            }
            let material_slot = ydd_part_material_slot(part);
            let properties_ref = part
                .get("properties_ref")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| properties_by_entry.get(entry).cloned())
                .or_else(|| root_properties_ref.clone())
                .or_else(|| request_properties_ref.clone());
            let descriptor_material_ref = properties_ref.as_ref().and_then(|properties_ref| {
                if !material_bindings_by_properties.contains_key(properties_ref) {
                    let bindings = self
                        .load_material_bindings_from_properties_ref(properties_ref)
                        .unwrap_or_default();
                    material_bindings_by_properties.insert(properties_ref.clone(), bindings);
                }
                material_bindings_by_properties
                    .get(properties_ref)
                    .and_then(|bindings| bindings.get(&material_slot).cloned())
            });
            out.push(self.decode_ydd_runtime_model_part(source, part, descriptor_material_ref)?);
        }
        if out.is_empty() {
            return Err(format!(
                "model.api: .ydd selector '{}' produced no runtime mesh parts path='{source}'",
                selector.unwrap_or("<all>")
            ));
        }
        Ok(out)
    }

    fn decode_ydd_runtime_model_part(
        &self,
        source: &str,
        part: &serde_json::Value,
        descriptor_material_ref: Option<String>,
    ) -> Result<ModelMeshPart, String> {
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
        let material_ref = descriptor_material_ref.or_else(|| {
            // Legacy fallback for pre-properties .ydd files. New .ydd assets must
            // declare material slots only and bind concrete .nemat refs through
            // their explicit .ytyp properties descriptor.
            part.get("material_ref")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
        });
        let material = match material_ref
            .as_deref()
            .and_then(|material_ref| self.load_material_binding_from_ref(material_ref))
        {
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
            mesh: PrimitiveMesh {
                vertices,
                indices,
                bounds_center,
                bounds_radius,
            },
            material,
        })
    }

    fn load_material_bindings_from_properties_ref(
        &self,
        properties_ref: &str,
    ) -> Result<std::collections::BTreeMap<String, String>, String> {
        let host = self.host.as_ref().ok_or_else(|| {
            format!(
                "model.api: .ydd properties_ref='{properties_ref}' requires engine.assets.definitions host access"
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
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            format!("model.api: .ytyp descriptor returned invalid json ref='{properties_ref}' err='{e}'")
        })?;
        let mut out = std::collections::BTreeMap::new();
        let Some(bindings) = value
            .get("model_explanation")
            .and_then(|value| value.get("material_bindings"))
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(out);
        };
        for binding in bindings {
            let slot = binding
                .get("slot")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let material_ref = binding
                .get("material_ref")
                .or_else(|| binding.get("material"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let (Some(slot), Some(material_ref)) = (slot, material_ref) {
                out.insert(slot.to_owned(), material_ref.to_owned());
            }
        }
        Ok(out)
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

fn ydd_part_material_slot(part: &serde_json::Value) -> String {
    part.get("material_slot")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("material")
        .to_owned()
}

fn ydd_root_properties_ref(root: &serde_json::Value) -> Option<String> {
    root.get("properties_ref")
        .or_else(|| root.get("descriptor_ref"))
        .or_else(|| root.get("ytyp_ref"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn ydd_properties_ref_by_entry(
    root: &serde_json::Value,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let Some(entries) = root.get("entries").and_then(serde_json::Value::as_array) else {
        return out;
    };
    for entry in entries {
        let name = entry
            .get("name")
            .or_else(|| entry.get("entry"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let properties_ref = entry
            .get("properties_ref")
            .or_else(|| entry.get("descriptor_ref"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let (Some(name), Some(properties_ref)) = (name, properties_ref) {
            out.insert(name.to_owned(), properties_ref.to_owned());
        }
    }
    out
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

fn json_vec3(
    value: Option<&serde_json::Value>,
    source: &str,
    index: usize,
    label: &str,
) -> Result<[f32; 3], String> {
    let arr = value.and_then(serde_json::Value::as_array).ok_or_else(|| {
        format!(
            "model.api: .ydd vertex field '{label}' must be vec3 path='{source}' vertex={index}"
        )
    })?;
    if arr.len() != 3 {
        return Err(format!("model.api: .ydd field '{label}' must have 3 components path='{source}' vertex={index} got={}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or_else(|| {
            format!("model.api: .ydd '{label}.x' must be number path='{source}' vertex={index}")
        })? as f32,
        arr[1].as_f64().ok_or_else(|| {
            format!("model.api: .ydd '{label}.y' must be number path='{source}' vertex={index}")
        })? as f32,
        arr[2].as_f64().ok_or_else(|| {
            format!("model.api: .ydd '{label}.z' must be number path='{source}' vertex={index}")
        })? as f32,
    ])
}

fn json_vec2(
    value: Option<&serde_json::Value>,
    source: &str,
    index: usize,
    label: &str,
) -> Result<[f32; 2], String> {
    let arr = value.and_then(serde_json::Value::as_array).ok_or_else(|| {
        format!(
            "model.api: .ydd vertex field '{label}' must be vec2 path='{source}' vertex={index}"
        )
    })?;
    if arr.len() != 2 {
        return Err(format!("model.api: .ydd field '{label}' must have 2 components path='{source}' vertex={index} got={}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or_else(|| {
            format!("model.api: .ydd '{label}.x' must be number path='{source}' vertex={index}")
        })? as f32,
        arr[1].as_f64().ok_or_else(|| {
            format!("model.api: .ydd '{label}.y' must be number path='{source}' vertex={index}")
        })? as f32,
    ])
}

fn json_vec4_value(value: &serde_json::Value) -> Result<[f32; 4], String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "vec4 value must be array".to_owned())?;
    if arr.len() != 4 {
        return Err(format!(
            "vec4 value must have 4 components, got {}",
            arr.len()
        ));
    }
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
