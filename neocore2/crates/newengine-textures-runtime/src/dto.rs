use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct TexturesServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub methods: &'static [&'static str],
    pub validation_policy: &'static str,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureRefRequest {
    pub texture_ref: String,
    pub dictionary_path: String,
    pub texture_name: Option<String>,
    pub texture_hash: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureManifestRequest {
    pub source: String,
    pub dictionary_path: String,
    pub texture_ref: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureRefValidation {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub logical_path: String,
    pub entry: Option<String>,
    pub texture_hash: Option<u64>,
    pub canonical: String,
    pub packet: Option<TexturePacketSummary>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TexturePacketSummary {
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub color_space: String,
    pub mip_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct StableDiagnostic {
    pub(crate) ok: bool,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) gateway: &'static str,
    pub(crate) byte_owner: &'static str,
    pub(crate) semantic_owner: &'static str,
}
