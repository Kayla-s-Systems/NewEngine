use std::collections::BTreeSet;

use newengine_assets::AssetServiceClient;
use newengine_assets_api::{AssetDecodeRequest, AssetDocumentRequest};
use serde_json::{json, Value};

use crate::model::{AssetInspectorReport, InspectorField};
use crate::source_pair::source_runtime_counterpart;

const DOMAIN_MANIFEST_OUTPUT: &str = "domain.manifest_json";

#[derive(Clone)]
pub(crate) struct NativeAssetInspector {
    client: AssetServiceClient,
}

impl NativeAssetInspector {
    pub(crate) fn new() -> Self {
        Self {
            client: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
        }
    }

    pub(crate) fn inspect(&self, asset_ref: &str) -> AssetInspectorReport {
        let asset_ref = normalize_ref(asset_ref);
        let mut report = AssetInspectorReport {
            asset_ref: asset_ref.clone(),
            title: file_name(&asset_ref),
            counterpart: source_runtime_counterpart(&asset_ref),
            ..AssetInspectorReport::default()
        };

        match self
            .client
            .inspect_document_json_v1(AssetDocumentRequest::new(asset_ref.clone()))
        {
            Ok(document) => {
                report.title = document.title;
                report.asset_kind = document.asset_kind;
                report.document_kind = document.document_kind;
                report.summary = document.preview.summary;
                report
                    .fields
                    .extend(document.sections.iter().flat_map(|section| {
                        section.fields.iter().map(|field| {
                            InspectorField::categorized(
                                section.title.clone(),
                                field.label.clone(),
                                compact_json(&field.value),
                            )
                        })
                    }));
                report.diagnostics.extend(
                    document
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.message),
                );
                if let Some(descriptor) = document.descriptor {
                    report.decoder = descriptor.handler_service.clone();
                    report.fields.push(InspectorField::categorized(
                        "Provider",
                        "Semantic gateway",
                        descriptor.semantic_gateway,
                    ));
                    report.fields.push(InspectorField::categorized(
                        "Provider",
                        "Native outputs",
                        descriptor.outputs.join(", "),
                    ));
                    if descriptor
                        .outputs
                        .iter()
                        .any(|it| it == DOMAIN_MANIFEST_OUTPUT)
                    {
                        self.inspect_native_runtime_manifest(&asset_ref, &mut report);
                    }
                }
            }
            Err(error) => report
                .diagnostics
                .push(format!("engine.assets.inspect: {error}")),
        }

        if source_extension(&asset_ref).is_some() {
            self.inspect_source(&asset_ref, &mut report);
        }
        if report.decoder.is_empty() {
            report.decoder = "engine.assets.raw_bytes_v1".to_owned();
        }
        if report.summary.is_empty() {
            report.summary = format!(
                "{} inspection fields · {} diagnostics",
                report.fields.len(),
                report.diagnostics.len()
            );
        }
        report
    }

    fn inspect_native_runtime_manifest(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        let (path, selector) = split_selector(asset_ref);
        let selector = selector.map_or_else(
            || json!({}),
            |entry| {
                json!({
                    "entry": entry,
                    "entry_name": entry,
                    "texture_name": entry,
                    "drawable_name": entry
                })
            },
        );
        let request = AssetDecodeRequest {
            logical_path: path.to_owned(),
            output_kind: DOMAIN_MANIFEST_OUTPUT.to_owned(),
            selector,
        };
        match self.client.decode_v1(&request) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => {
                    report.decoder = format!("{} → {DOMAIN_MANIFEST_OUTPUT}", report.decoder);
                    append_json_summary("Native manifest", &value, &mut report.fields);
                }
                Err(error) => report
                    .diagnostics
                    .push(format!("native manifest JSON decode failed: {error}")),
            },
            Err(error) => report
                .diagnostics
                .push(format!("native manifest unavailable: {error}")),
        }
    }

    fn inspect_source(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        let extension = source_extension(asset_ref).unwrap_or_default();
        match extension.as_str() {
            "xml" | "ymap.xml" | "ytyp.xml" | "nemat.xml" | "neui.xml" | "svg" => {
                self.inspect_xml(asset_ref, report)
            }
            "json" => self.inspect_json(asset_ref, report),
            "dds" => self.inspect_dds(asset_ref, report),
            "obj" => self.inspect_obj(asset_ref, report),
            "mtl" => self.inspect_mtl(asset_ref, report),
            "gltf" => self.inspect_gltf(asset_ref, report),
            "glb" => self.inspect_glb(asset_ref, report),
            "png" | "bmp" | "jpg" | "jpeg" => {
                self.inspect_raster_header(asset_ref, &extension, report)
            }
            "ttf" => self.inspect_ttf(asset_ref, report),
            "spv" => self.inspect_spirv(asset_ref, report),
            "fbx" => self.inspect_fbx(asset_ref, report),
            "bin" => self.inspect_binary_buffer(asset_ref, report),
            "txt" | "md" | "vert" | "frag" | "comp" | "glsl" => {
                self.inspect_text(asset_ref, report)
            }
            _ => self.inspect_binary(asset_ref, report),
        }
    }

    fn inspect_xml(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.text_v1(asset_ref) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => match newengine_authored_xml::parse_xml_document(&text, asset_ref) {
                    Ok(document) => {
                        let root = document.root_element();
                        let node_count = document
                            .descendants()
                            .filter(|node| node.is_element())
                            .count();
                        let attribute_count = document
                            .descendants()
                            .filter(|node| node.is_element())
                            .map(|node| node.attributes().len())
                            .sum::<usize>();
                        let tags = document
                            .descendants()
                            .filter(|node| node.is_element())
                            .map(|node| node.tag_name().name().to_owned())
                            .collect::<BTreeSet<_>>();
                        report.decoder = "newengine-authored-xml".to_owned();
                        report.asset_kind = "authored_xml".to_owned();
                        report.fields.extend([
                            InspectorField::new("Root", root.tag_name().name()),
                            InspectorField::new(
                                "Schema",
                                root.attribute("schema").unwrap_or("<not declared>"),
                            ),
                            InspectorField::new("Elements", node_count.to_string()),
                            InspectorField::new("Attributes", attribute_count.to_string()),
                            InspectorField::new(
                                "Tags",
                                tags.into_iter().take(20).collect::<Vec<_>>().join(", "),
                            ),
                            InspectorField::new("UTF-8 bytes", text.len().to_string()),
                        ]);
                    }
                    Err(error) => report.diagnostics.push(error),
                },
                Err(error) => report
                    .diagnostics
                    .push(format!("source is not UTF-8: {error}")),
            },
            Err(error) => report
                .diagnostics
                .push(format!("engine.assets.text_v1: {error}")),
        }
    }

    fn inspect_json(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.text_v1(asset_ref) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => {
                    report.decoder = "serde_json native source parser".to_owned();
                    report.asset_kind = "authored_json".to_owned();
                    append_json_summary("JSON", &value, &mut report.fields);
                    report
                        .fields
                        .push(InspectorField::new("UTF-8 bytes", bytes.len().to_string()));
                }
                Err(error) => report
                    .diagnostics
                    .push(format!("JSON parse failed: {error}")),
            },
            Err(error) => report
                .diagnostics
                .push(format!("engine.assets.text_v1: {error}")),
        }
    }

    fn inspect_dds(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => match newengine_texture_container::read_dds_runtime_texture(&bytes) {
                Ok(texture) => {
                    let payload_bytes = texture
                        .mips
                        .iter()
                        .map(|mip| mip.bytes.len())
                        .sum::<usize>();
                    report.decoder =
                        "newengine-texture-container::read_dds_runtime_texture".to_owned();
                    report.asset_kind = "source_texture".to_owned();
                    report.fields.extend([
                        InspectorField::new(
                            "Extent",
                            format!("{} × {}", texture.width, texture.height),
                        ),
                        InspectorField::new("Format", texture.format),
                        InspectorField::new("Color space", texture.color_space),
                        InspectorField::new("Mip levels", texture.mips.len().to_string()),
                        InspectorField::new("Runtime payload bytes", payload_bytes.to_string()),
                        InspectorField::new("Source bytes", bytes.len().to_string()),
                    ]);
                }
                Err(error) => report.diagnostics.push(error.to_string()),
            },
            Err(error) => report
                .diagnostics
                .push(format!("engine.assets.raw_bytes_v1: {error}")),
        }
    }

    fn inspect_obj(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        let text = match self
            .client
            .text_v1(asset_ref)
            .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
        {
            Ok(text) => text,
            Err(error) => {
                report.diagnostics.push(error);
                return;
            }
        };
        let client = self.client.clone();
        match newengine_model_import_obj::decode_obj_with_mtl_loader(
            asset_ref,
            &text,
            1.0,
            move |path| {
                client
                    .text_v1(path)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            },
        ) {
            Ok(decoded) => {
                let vertices = decoded
                    .parts
                    .iter()
                    .map(|part| part.mesh.vertices.len())
                    .sum::<usize>();
                let indices = decoded
                    .parts
                    .iter()
                    .map(|part| part.mesh.indices.len())
                    .sum::<usize>();
                report.decoder = "newengine-model-import-obj".to_owned();
                report.asset_kind = "source_model".to_owned();
                report.fields.extend([
                    InspectorField::new("Mesh parts", decoded.parts.len().to_string()),
                    InspectorField::new("Vertices", vertices.to_string()),
                    InspectorField::new("Indices", indices.to_string()),
                    InspectorField::new("Triangles", (indices / 3).to_string()),
                    InspectorField::new("Materials", decoded.materials.len().to_string()),
                    InspectorField::new("MTL libraries", decoded.mtllibs.join(", ")),
                ]);
            }
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_mtl(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.text_v1(asset_ref) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let base = asset_ref.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
                    let materials = newengine_model_import_obj::parse_mtl_text(base, &text);
                    report.decoder = "newengine-model-import-obj::parse_mtl_text".to_owned();
                    report.asset_kind = "source_material_library".to_owned();
                    report.fields.push(InspectorField::new(
                        "Materials",
                        materials.keys().cloned().collect::<Vec<_>>().join(", "),
                    ));
                }
                Err(error) => report.diagnostics.push(error.to_string()),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_gltf(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.text_v1(asset_ref) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => {
                    report.decoder = "native glTF JSON inspection".to_owned();
                    report.asset_kind = "source_model".to_owned();
                    append_gltf_summary(&value, &mut report.fields);
                }
                Err(error) => report
                    .diagnostics
                    .push(format!("glTF JSON parse failed: {error}")),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_glb(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => match glb_json_chunk(&bytes) {
                Ok(json_bytes) => match serde_json::from_slice::<Value>(json_bytes) {
                    Ok(value) => {
                        report.decoder = "native GLB v2 chunk parser".to_owned();
                        report.asset_kind = "source_model".to_owned();
                        append_gltf_summary(&value, &mut report.fields);
                        report.fields.push(InspectorField::new(
                            "Container bytes",
                            bytes.len().to_string(),
                        ));
                    }
                    Err(error) => report.diagnostics.push(error.to_string()),
                },
                Err(error) => report.diagnostics.push(error),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_raster_header(
        &self,
        asset_ref: &str,
        extension: &str,
        report: &mut AssetInspectorReport,
    ) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => {
                let extent = match extension {
                    "png" => png_extent(&bytes),
                    "bmp" => bmp_extent(&bytes),
                    "jpg" | "jpeg" => jpeg_extent(&bytes),
                    _ => None,
                };
                report.decoder = format!("native {extension} header parser");
                report.asset_kind = "source_texture".to_owned();
                report.fields.push(InspectorField::new(
                    "Extent",
                    extent
                        .map(|[w, h]| format!("{w} × {h}"))
                        .unwrap_or_else(|| "unknown".to_owned()),
                ));
                report
                    .fields
                    .push(InspectorField::new("Source bytes", bytes.len().to_string()));
            }
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_ttf(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => match ttf_summary(&bytes) {
                Ok((flavor, tables)) => {
                    report.decoder = "native SFNT/TTF header parser".to_owned();
                    report.asset_kind = "source_font".to_owned();
                    report.fields.extend([
                        InspectorField::new("SFNT flavor", flavor),
                        InspectorField::new("Tables", tables.to_string()),
                        InspectorField::new("Source bytes", bytes.len().to_string()),
                    ]);
                }
                Err(error) => report.diagnostics.push(error),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_spirv(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => match spirv_summary(&bytes) {
                Ok(summary) => {
                    report.decoder = "native SPIR-V header parser".to_owned();
                    report.asset_kind = "shader_binary".to_owned();
                    report.fields.extend(summary);
                }
                Err(error) => report.diagnostics.push(error),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_fbx(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => {
                report.asset_kind = "source_model".to_owned();
                if bytes.starts_with(b"Kaydara FBX Binary  \0\x1a\0") && bytes.len() >= 27 {
                    let version = u32::from_le_bytes(bytes[23..27].try_into().unwrap());
                    report.decoder = "native FBX binary header parser".to_owned();
                    report.fields.extend([
                        InspectorField::new("Encoding", "binary"),
                        InspectorField::new("Version", version.to_string()),
                        InspectorField::new("Source bytes", bytes.len().to_string()),
                    ]);
                } else {
                    report.decoder = "native FBX ASCII probe".to_owned();
                    let text = String::from_utf8_lossy(&bytes);
                    report.fields.extend([
                        InspectorField::new("Encoding", "ASCII / unknown"),
                        InspectorField::new("Lines", text.lines().count().to_string()),
                        InspectorField::new("Source bytes", bytes.len().to_string()),
                    ]);
                }
            }
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_binary_buffer(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => {
                report.decoder = "native binary-buffer inspector".to_owned();
                report.asset_kind = "source_binary_buffer".to_owned();
                let nonzero = bytes.iter().filter(|byte| **byte != 0).count();
                report.fields.extend([
                    InspectorField::new("Bytes", bytes.len().to_string()),
                    InspectorField::new("Non-zero bytes", nonzero.to_string()),
                    InspectorField::new(
                        "Magic",
                        bytes
                            .iter()
                            .take(16)
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                ]);
            }
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_text(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.text_v1(asset_ref) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    report.decoder = "engine.assets.text_v1".to_owned();
                    report.asset_kind = "source_text".to_owned();
                    report.fields.extend([
                        InspectorField::new("Lines", text.lines().count().to_string()),
                        InspectorField::new("Characters", text.chars().count().to_string()),
                        InspectorField::new("UTF-8 bytes", text.len().to_string()),
                    ]);
                }
                Err(error) => report.diagnostics.push(error.to_string()),
            },
            Err(error) => report.diagnostics.push(error),
        }
    }

    fn inspect_binary(&self, asset_ref: &str, report: &mut AssetInspectorReport) {
        match self.client.raw_bytes_v1(asset_ref) {
            Ok(bytes) => {
                report.fields.extend([
                    InspectorField::new("Bytes", bytes.len().to_string()),
                    InspectorField::new(
                        "Magic",
                        bytes
                            .iter()
                            .take(16)
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                ]);
            }
            Err(error) => report.diagnostics.push(error),
        }
    }
}

fn append_gltf_summary(value: &Value, fields: &mut Vec<InspectorField>) {
    for (label, key) in [
        ("Scenes", "scenes"),
        ("Nodes", "nodes"),
        ("Meshes", "meshes"),
        ("Materials", "materials"),
        ("Textures", "textures"),
        ("Images", "images"),
        ("Animations", "animations"),
        ("Skins", "skins"),
    ] {
        fields.push(InspectorField::new(
            label,
            array_len(value, key).to_string(),
        ));
    }
}

fn append_json_summary(category: &str, value: &Value, fields: &mut Vec<InspectorField>) {
    match value {
        Value::Object(map) => {
            fields.push(InspectorField::categorized(
                category,
                "Object fields",
                map.len().to_string(),
            ));
            fields.push(InspectorField::categorized(
                category,
                "Top keys",
                map.keys().take(24).cloned().collect::<Vec<_>>().join(", "),
            ));
            for (key, value) in map.iter().take(12) {
                fields.push(InspectorField::categorized(
                    category,
                    key,
                    compact_json(value),
                ));
            }
        }
        Value::Array(items) => fields.push(InspectorField::categorized(
            category,
            "Array elements",
            items.len().to_string(),
        )),
        other => fields.push(InspectorField::categorized(
            category,
            "Value",
            compact_json(other),
        )),
    }
}

fn compact_json(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_owned(),
        other => {
            let text = other.to_string();
            if text.chars().count() <= 180 {
                text
            } else {
                format!("{}…", text.chars().take(180).collect::<String>())
            }
        }
    }
}

fn source_extension(asset_ref: &str) -> Option<String> {
    let path = asset_ref
        .split('@')
        .next()
        .unwrap_or(asset_ref)
        .to_ascii_lowercase();
    for compound in ["ymap.xml", "ytyp.xml", "nemat.xml", "neui.xml"] {
        if path.ends_with(compound) {
            return Some(compound.to_owned());
        }
    }
    let extension = path.rsplit_once('.')?.1;
    const SOURCE_EXTENSIONS: &[&str] = &[
        "xml", "json", "dds", "obj", "mtl", "gltf", "glb", "png", "bmp", "jpg", "jpeg", "svg",
        "txt", "md", "vert", "frag", "comp", "glsl", "ttf", "fbx", "bin", "spv",
    ];
    SOURCE_EXTENSIONS
        .contains(&extension)
        .then(|| extension.to_owned())
}

fn normalize_ref(value: &str) -> String {
    value.trim().replace('\\', "/").trim_matches('/').to_owned()
}

fn file_name(value: &str) -> String {
    value.rsplit('/').next().unwrap_or(value).to_owned()
}

fn split_selector(value: &str) -> (&str, Option<&str>) {
    value
        .split_once('@')
        .map_or((value, None), |(path, selector)| (path, Some(selector)))
}

fn array_len(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

fn glb_json_chunk(bytes: &[u8]) -> Result<&[u8], String> {
    if bytes.len() < 20 || bytes.get(..4) != Some(b"glTF") {
        return Err("GLB header missing glTF magic".to_owned());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported GLB version {version}"));
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if declared > bytes.len() {
        return Err("GLB declared length exceeds payload".to_owned());
    }
    let chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let chunk_kind = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    if chunk_kind != 0x4E4F_534A {
        return Err("GLB first chunk is not JSON".to_owned());
    }
    bytes
        .get(20..20usize.saturating_add(chunk_len))
        .ok_or_else(|| "GLB JSON chunk is truncated".to_owned())
}

fn png_extent(bytes: &[u8]) -> Option<[u32; 2]> {
    (bytes.len() >= 24 && bytes.get(..8) == Some(b"\x89PNG\r\n\x1a\n")).then(|| {
        [
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        ]
    })
}

fn jpeg_extent(bytes: &[u8]) -> Option<[u32; 2]> {
    if bytes.len() < 4 || bytes.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    let mut offset = 2usize;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xFF {
            offset += 1;
            continue;
        }
        while offset < bytes.len() && bytes[offset] == 0xFF {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let height =
                u16::from_be_bytes(bytes.get(offset + 3..offset + 5)?.try_into().ok()?) as u32;
            let width =
                u16::from_be_bytes(bytes.get(offset + 5..offset + 7)?.try_into().ok()?) as u32;
            return Some([width, height]);
        }
        offset += length;
    }
    None
}

fn ttf_summary(bytes: &[u8]) -> Result<(String, u16), String> {
    if bytes.len() < 12 {
        return Err("font SFNT header is truncated".to_owned());
    }
    let flavor = match bytes.get(..4) {
        Some([0x00, 0x01, 0x00, 0x00]) => "TrueType 1.0",
        Some(b"OTTO") => "OpenType CFF",
        Some(b"true") => "Apple TrueType",
        Some(b"ttcf") => "TrueType Collection",
        _ => return Err("unsupported SFNT font magic".to_owned()),
    };
    let tables = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
    Ok((flavor.to_owned(), tables))
}

fn spirv_summary(bytes: &[u8]) -> Result<Vec<InspectorField>, String> {
    if bytes.len() < 20 || !bytes.len().is_multiple_of(4) {
        return Err("SPIR-V payload is truncated or not word aligned".to_owned());
    }
    let word = |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    if word(0) != 0x0723_0203 {
        return Err("SPIR-V magic mismatch".to_owned());
    }
    let version = word(4);
    let major = (version >> 16) & 0xFF;
    let minor = (version >> 8) & 0xFF;
    Ok(vec![
        InspectorField::new("Version", format!("{major}.{minor}")),
        InspectorField::new("Generator", format!("0x{:08X}", word(8))),
        InspectorField::new("ID bound", word(12).to_string()),
        InspectorField::new("Schema", word(16).to_string()),
        InspectorField::new("Words", (bytes.len() / 4).to_string()),
        InspectorField::new("Bytes", bytes.len().to_string()),
    ])
}

fn bmp_extent(bytes: &[u8]) -> Option<[u32; 2]> {
    (bytes.len() >= 26 && bytes.get(..2) == Some(b"BM")).then(|| {
        [
            u32::from_le_bytes(bytes[18..22].try_into().unwrap()),
            u32::from_le_bytes(bytes[22..26].try_into().unwrap()),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_glb_json_chunk() {
        let json = br#"{"asset":{"version":"2.0"}} "#;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&(20u32 + json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        bytes.extend_from_slice(json);
        assert_eq!(glb_json_chunk(&bytes).unwrap(), json);
    }

    #[test]
    fn parses_spirv_header() {
        let words = [0x0723_0203u32, 0x0001_0600, 0x0008_000B, 42, 0];
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let fields = spirv_summary(&bytes).expect("SPIR-V");
        assert!(fields.iter().any(|field| field.value == "1.6"));
    }

    #[test]
    fn parses_truetype_header() {
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x0C];
        bytes.resize(12, 0);
        assert_eq!(
            ttf_summary(&bytes).unwrap(),
            ("TrueType 1.0".to_owned(), 12)
        );
    }

    #[test]
    fn detects_compound_authored_extensions() {
        assert_eq!(
            source_extension("maps/source/a.ymap.xml").as_deref(),
            Some("ymap.xml")
        );
    }
}
