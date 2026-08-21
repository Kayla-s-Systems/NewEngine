use newengine_authored_xml as authored_xml;
use newengine_materials::api::material_id_from_name;
use newengine_materials::{
    validate_authored_material_library, validate_material_texture_reference,
    AuthoredMaterialDescriptor, AuthoredMaterialLibrary, MaterialDescriptor, MaterialDomain,
    MaterialFlags, MaterialLoadResponse, MaterialParamValue, MaterialTextureBindings, ShadingModel,
};

#[inline]
pub(crate) fn material_cache_key(source: &str, selector: &str) -> String {
    format!("{}@{}", source.trim().replace('\\', "/"), selector.trim())
}

#[inline]
pub(crate) fn split_nemat_selector(
    logical_path: &str,
    request_selector: Option<&str>,
) -> Result<(String, String), String> {
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
        return Err(format!(
            "materials: invalid .nemat entry selector '{selector}'"
        ));
    }
    let source = normalize_material_logical_path(path_part)?;
    Ok((source, selector.to_owned()))
}

pub fn decode_nemat_material_library_from_body(
    bytes: &[u8],
) -> Result<AuthoredMaterialLibrary, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        "NEMAT payload must be UTF-8 XML material library inside the NEF8 ListFile body".to_owned()
    })?;
    if !authored_xml::text_is_xml(text) {
        return Err(
            "NEMAT body must be XML <NematMaterialLibrary>; JSON material bodies are forbidden in authored .nemat files"
                .to_owned(),
        );
    }
    let library = decode_nemat_material_library_xml(text)?;
    let validation = validate_authored_material_library(&library);
    if !validation.valid {
        return Err(format!(
            "invalid XML material library: {}",
            validation.errors.join("; ")
        ));
    }
    Ok(library)
}

pub(crate) fn preview_material_name_from_body(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        "NEMAT payload must be UTF-8 XML material library inside the NEF8 ListFile body".to_owned()
    })?;
    if !authored_xml::text_is_xml(text) {
        return Err(
            "NEMAT body must be XML <NematMaterialLibrary>; JSON material bodies are forbidden in authored .nemat files"
                .to_owned(),
        );
    }
    let library = decode_nemat_material_library_xml(text)?;
    let validation = validate_authored_material_library(&library);
    if !validation.valid {
        return Err(format!(
            "invalid XML material library: {}",
            validation.errors.join("; ")
        ));
    }

    library
        .materials
        .iter()
        .enumerate()
        .filter(|(_, material)| !material.name.trim().is_empty())
        .max_by_key(|(index, material)| {
            // A root .nemat preview is a visual thumbnail, not semantic entry
            // selection. Prefer a material that can visibly demonstrate the
            // library: opaque first, then base-color-textured, then any textured
            // material. Preserve authored order as the final tie-breaker.
            let blend = material.surface.blend.trim().to_ascii_lowercase();
            let opaque = matches!(blend.as_str(), "" | "opaque");
            let has_base_color = material.textures.keys().any(|slot| {
                matches!(
                    slot.trim().to_ascii_lowercase().as_str(),
                    "base_color" | "basecolor" | "albedo" | "diffuse"
                )
            });
            let base_alpha = material
                .params
                .get("base_color")
                .and_then(|value| match value {
                    MaterialParamValue::Color(value) | MaterialParamValue::Float4(value) => {
                        Some(value[3])
                    }
                    _ => None,
                })
                .unwrap_or(1.0);
            let visible_alpha = base_alpha >= 0.95;
            let textured = !material.textures.is_empty();
            let conventional_name = matches!(
                material.name.trim().to_ascii_lowercase().as_str(),
                "material" | "default" | "main" | "body"
            );
            (
                opaque as u8,
                visible_alpha as u8,
                has_base_color as u8,
                textured as u8,
                conventional_name as u8,
                usize::MAX - *index,
            )
        })
        .map(|(_, material)| material.name.trim().to_owned())
        .ok_or_else(|| "material library contains no named materials".to_owned())
}

pub(crate) fn decode_material_entry_payload(
    bytes: &[u8],
    selector: &str,
) -> Result<AuthoredMaterialDescriptor, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        "NEMAT payload must be UTF-8 XML material library inside the NEF8 ListFile body".to_owned()
    })?;
    if !authored_xml::text_is_xml(text) {
        return Err("NEMAT body must be XML <NematMaterialLibrary>; JSON material bodies are forbidden in authored .nemat files".to_owned());
    }
    let library = decode_nemat_material_library_xml(text)?;
    let validation = validate_authored_material_library(&library);
    if !validation.valid {
        return Err(format!(
            "invalid XML material library: {}",
            validation.errors.join("; ")
        ));
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
    if !schema.is_empty()
        && schema != newengine_asset_format_nef8::nemat::AUTHORED_XML_SCHEMA
        && !newengine_asset_format_nef8::nemat::LEGACY_AUTHORED_XML_SCHEMAS
            .iter()
            .any(|legacy| *legacy == schema)
    {
        return Err(format!(
            "unsupported NEMAT authored XML schema '{schema}', expected '{}' or readable legacy [{}]",
            newengine_asset_format_nef8::nemat::AUTHORED_XML_SCHEMA,
            newengine_asset_format_nef8::nemat::LEGACY_AUTHORED_XML_SCHEMAS.join(",")
        ));
    }
    let mut library = AuthoredMaterialLibrary {
        version: authored_xml::xml_attr_u32_any(root, &["version"]).unwrap_or(1),
        materials: Vec::new(),
    };
    for material_node in root
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("Material"))
    {
        library
            .materials
            .push(decode_nemat_material_xml(material_node)?);
    }
    Ok(library)
}

fn decode_nemat_material_xml(
    node: authored_xml::XmlNode<'_, '_>,
) -> Result<AuthoredMaterialDescriptor, String> {
    let mut material = AuthoredMaterialDescriptor {
        name: authored_xml::xml_attr_any(node, &["name", "id"]).unwrap_or_default(),
        shader: authored_xml::xml_attr_any(node, &["shader", "shader_id", "shaderId"])
            .unwrap_or_else(|| "pbr.default".to_owned()),
        ..AuthoredMaterialDescriptor::default()
    };
    if material.name.trim().is_empty() {
        return Err("NEMAT XML <Material> entry missing name".to_owned());
    }
    if let Some(surface) = authored_xml::xml_child(node, "Surface") {
        material.surface.blend =
            authored_xml::xml_attr_any(surface, &["blend"]).unwrap_or_else(|| "opaque".to_owned());
        material.surface.two_sided = authored_xml::xml_attr_bool_any(
            surface,
            &["two_sided", "twoSided", "double_sided", "doubleSided"],
        )
        .unwrap_or(false);
        material.surface.alpha_cutoff =
            authored_xml::xml_attr_f32_any(surface, &["alpha_cutoff", "alphaCutoff"]);
    }
    if let Some(textures) = authored_xml::xml_child(node, "Textures") {
        for texture in textures
            .children()
            .filter(|child| child.is_element() && child.has_tag_name("Texture"))
        {
            let slot = authored_xml::xml_attr_any(texture, &["slot", "name"]).unwrap_or_default();
            let reference = authored_xml::xml_attr_any(
                texture,
                &["ref", "reference", "texture_ref", "textureRef"],
            )
            .unwrap_or_default();
            if !slot.trim().is_empty() && !reference.trim().is_empty() {
                material.textures.insert(slot, reference);
            }
        }
    }
    if let Some(params) = authored_xml::xml_child(node, "Params") {
        for param in params
            .children()
            .filter(|child| child.is_element() && child.has_tag_name("Param"))
        {
            let name = authored_xml::xml_attr_any(param, &["name", "key"]).unwrap_or_default();
            if name.trim().is_empty() {
                continue;
            }
            let kind = authored_xml::xml_attr_any(param, &["type", "kind"])
                .unwrap_or_else(|| "float".to_owned());
            let raw = authored_xml::xml_attr_any(param, &["value", "ref", "reference"])
                .or_else(|| {
                    param
                        .text()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_default();
            material
                .params
                .insert(name, parse_material_param_value(&kind, &raw)?);
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
        "int" | "i32" => raw
            .trim()
            .parse::<i32>()
            .map(MaterialParamValue::Int)
            .map_err(|e| format!("material int param parse failed value='{raw}' err='{e}'")),
        "bool" | "boolean" => Ok(MaterialParamValue::Bool(matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ))),
        "enum" => Ok(MaterialParamValue::Enum(raw.trim().to_owned())),
        "texture_ref" | "texture" => Ok(MaterialParamValue::TextureRef(raw.trim().to_owned())),
        other => Err(format!(
            "unsupported material XML param type '{other}' value='{raw}'"
        )),
    }
}

fn parse_f32(raw: &str) -> Result<f32, String> {
    raw.trim()
        .parse::<f32>()
        .map_err(|e| format!("material float param parse failed value='{raw}' err='{e}'"))
}

fn parse_f32_array<const N: usize>(raw: &str) -> Result<[f32; N], String> {
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(parse_f32)
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != N {
        return Err(format!(
            "material vector param expected {N} components, got {} value='{raw}'",
            values.len()
        ));
    }
    let mut out = [0.0; N];
    out.copy_from_slice(&values);
    Ok(out)
}

pub(crate) fn material_response_from_authored(
    source: &str,
    selector: &str,
    material: AuthoredMaterialDescriptor,
) -> Result<MaterialLoadResponse, String> {
    let mut descriptor = descriptor_from_authored(&material);
    descriptor.sanitize_in_place();
    let textures = texture_bindings_from_authored(&material)?;
    let name = if material.name.trim().is_empty() {
        selector.to_owned()
    } else {
        material.name
    };
    Ok(MaterialLoadResponse {
        source: format!("{source}@{selector}"),
        id: material_id_from_name(&name),
        name,
        descriptor,
        textures,
    })
}

fn descriptor_from_authored(material: &AuthoredMaterialDescriptor) -> MaterialDescriptor {
    let mut descriptor = MaterialDescriptor {
        domain: MaterialDomain::Surface,
        shading_model: if material.shader.to_ascii_lowercase().contains("unlit") {
            ShadingModel::Unlit
        } else {
            ShadingModel::PbrMetallicRoughness
        },
        ..MaterialDescriptor::default()
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

fn texture_bindings_from_authored(
    material: &AuthoredMaterialDescriptor,
) -> Result<MaterialTextureBindings, String> {
    let mut bindings = MaterialTextureBindings::default();
    for (slot, reference) in &material.textures {
        let canonical = validate_material_texture_reference(reference)
            .map_err(|e| {
                format!(
                    "material '{}' texture slot '{}' invalid: {}",
                    material.name, slot, e
                )
            })?
            .canonical;
        match slot.as_str() {
            "base_color" | "albedo" | "diffuse" => bindings.base_color_texture = Some(canonical),
            "normal" | "normal_map" => bindings.normal_texture = Some(canonical),
            "metallic" => bindings.metallic_texture = Some(canonical),
            "roughness" => bindings.roughness_texture = Some(canonical),
            "occlusion" | "ao" => bindings.occlusion_texture = Some(canonical),
            "emissive" => bindings.emissive_texture = Some(canonical),
            other => {
                return Err(format!(
                    "material '{}' has unknown texture slot '{}'",
                    material.name, other
                ))
            }
        }
    }
    if let Some(value) = param_float2(&material.params, "uv_scale") {
        bindings.uv_scale = value;
    }
    if let Some(value) = param_float2(&material.params, "uv_offset") {
        bindings.uv_offset = value;
    }
    Ok(bindings.sanitized())
}

fn param_f32(
    params: &std::collections::BTreeMap<String, MaterialParamValue>,
    key: &str,
) -> Option<f32> {
    match params.get(key)? {
        MaterialParamValue::Float(value) => Some(*value),
        MaterialParamValue::Int(value) => Some(*value as f32),
        _ => None,
    }
}

fn param_color(
    params: &std::collections::BTreeMap<String, MaterialParamValue>,
    key: &str,
) -> Option<[f32; 4]> {
    match params.get(key)? {
        MaterialParamValue::Color(value) | MaterialParamValue::Float4(value) => Some(*value),
        MaterialParamValue::Float3(value) => Some([value[0], value[1], value[2], 1.0]),
        _ => None,
    }
}

fn param_float2(
    params: &std::collections::BTreeMap<String, MaterialParamValue>,
    key: &str,
) -> Option<[f32; 2]> {
    match params.get(key)? {
        MaterialParamValue::Float2(value) => Some(*value),
        _ => None,
    }
}

fn param_float3(
    params: &std::collections::BTreeMap<String, MaterialParamValue>,
    key: &str,
) -> Option<[f32; 3]> {
    match params.get(key)? {
        MaterialParamValue::Float3(value) => Some(*value),
        MaterialParamValue::Color(value) | MaterialParamValue::Float4(value) => {
            Some([value[0], value[1], value[2]])
        }
        _ => None,
    }
}

pub(crate) fn collect_texture_refs(textures: &MaterialTextureBindings) -> Vec<&str> {
    let mut out = Vec::new();
    if let Some(value) = textures.base_color_texture.as_deref() {
        out.push(value);
    }
    if let Some(value) = textures.normal_texture.as_deref() {
        out.push(value);
    }
    if let Some(value) = textures.metallic_texture.as_deref() {
        out.push(value);
    }
    if let Some(value) = textures.roughness_texture.as_deref() {
        out.push(value);
    }
    if let Some(value) = textures.occlusion_texture.as_deref() {
        out.push(value);
    }
    if let Some(value) = textures.emissive_texture.as_deref() {
        out.push(value);
    }
    out
}

pub(crate) fn normalize_material_logical_path(path: &str) -> Result<String, String> {
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
