#![forbid(unsafe_op_in_unsafe_fn)]

use crate::types::Asset;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model3dFormat {
    Obj,
    Fbx,
    Glb,
    Gltf,
    Ne3d,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Model3dMeta {
    pub schema: String,
    pub source: String,
    pub container: String,
    pub payload_format: String,

    pub meshes: u32,
    pub vertices: u64,
    pub indices: u64,

    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct Model3dAsset {
    pub format: Model3dFormat,
    pub meta: Model3dMeta,
    pub payload: Vec<u8>,
}

impl Asset for Model3dAsset {
    #[inline]
    fn type_name() -> &'static str {
        "Model3dAsset"
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Model3dReadError {
    #[error("wire: too short")]
    TooShort,
    #[error("wire: meta length out of bounds")]
    MetaOutOfBounds,
    #[error("wire: meta length too large ({0} bytes)")]
    MetaTooLarge(usize),
    #[error("utf8: {0}")]
    Utf8(String),
    #[error("meta json: {0}")]
    MetaJson(String),
}

pub struct Model3dReader;

impl Model3dReader {
    pub const MAX_META_BYTES: usize = 256 * 1024;

    pub fn from_blob_parts(meta_json: &str, payload: &[u8]) -> Result<Model3dAsset, Model3dReadError> {
        let meta = parse_meta_json(meta_json)?;
        let format = detect_format(&meta.container, &meta.payload_format);
        Ok(Model3dAsset {
            format,
            meta,
            payload: payload.to_vec(),
        })
    }

    /// Decodes importer wire:
    /// [4] meta_len_le (u32)
    /// [N] meta_json utf8
    /// [..] payload bytes (rest)
    pub fn read_wire(bytes: &[u8]) -> Result<Model3dAsset, Model3dReadError> {
        if bytes.len() < 4 {
            return Err(Model3dReadError::TooShort);
        }

        let meta_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if meta_len > Self::MAX_META_BYTES {
            return Err(Model3dReadError::MetaTooLarge(meta_len));
        }

        let meta_start = 4usize;
        let meta_end = meta_start.saturating_add(meta_len);
        if meta_end > bytes.len() {
            return Err(Model3dReadError::MetaOutOfBounds);
        }

        let meta_bytes = &bytes[meta_start..meta_end];
        let payload = &bytes[meta_end..];

        let meta_str =
            std::str::from_utf8(meta_bytes).map_err(|e| Model3dReadError::Utf8(e.to_string()))?;

        Self::from_blob_parts(meta_str, payload)
    }
}

fn parse_meta_json(meta_json: &str) -> Result<Model3dMeta, Model3dReadError> {
    let v: JsonValue =
        serde_json::from_str(meta_json).map_err(|e| Model3dReadError::MetaJson(e.to_string()))?;

    let schema = v.get("schema").and_then(|x| x.as_str()).unwrap_or("").to_owned();

    let source = v.get("source").and_then(|x| x.as_str()).unwrap_or("").to_owned();

    let container_raw = v.get("container").and_then(|x| x.as_str()).unwrap_or("").to_owned();
    let container = normalize_container(&container_raw);

    let payload_format = v
        .get("payload_format")
        .or_else(|| v.get("payload"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_owned();

    let meshes = v.get("meshes").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let vertices = v.get("vertices").and_then(|x| x.as_u64()).unwrap_or(0);
    let indices = v.get("indices").and_then(|x| x.as_u64()).unwrap_or(0);

    let bbox_min = parse_vec3(v.get("bbox_min")).unwrap_or([0.0, 0.0, 0.0]);
    let bbox_max = parse_vec3(v.get("bbox_max")).unwrap_or([0.0, 0.0, 0.0]);

    Ok(Model3dMeta {
        schema,
        source,
        container,
        payload_format,
        meshes,
        vertices,
        indices,
        bbox_min,
        bbox_max,
    })
}

#[inline]
fn parse_vec3(v: Option<&JsonValue>) -> Option<[f32; 3]> {
    let a = v?.as_array()?;
    if a.len() < 3 {
        return None;
    }
    let x = a[0].as_f64()? as f32;
    let y = a[1].as_f64()? as f32;
    let z = a[2].as_f64()? as f32;
    Some([x, y, z])
}

#[inline]
fn normalize_container(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "obj" => "obj".to_owned(),
        "fbx" => "fbx".to_owned(),
        "glb" => "glb".to_owned(),
        "gltf" => "gltf".to_owned(),
        "ne3d" => "ne3d".to_owned(),
        other => other.to_owned(),
    }
}

#[inline]
fn detect_format(container: &str, payload_format: &str) -> Model3dFormat {
    let c = container.trim().to_ascii_lowercase();
    let p = payload_format.trim().to_ascii_lowercase();

    if p == "ne3d" {
        return Model3dFormat::Ne3d;
    }

    match c.as_str() {
        "obj" => Model3dFormat::Obj,
        "fbx" => Model3dFormat::Fbx,
        "glb" => Model3dFormat::Glb,
        "gltf" => Model3dFormat::Gltf,
        _ => Model3dFormat::Unknown,
    }
}
