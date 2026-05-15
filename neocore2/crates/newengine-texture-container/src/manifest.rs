use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureDictionaryManifest {
    pub schema: String,
    pub version: u16,
    pub default_format: String,
    pub entries: Vec<TextureEntryMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureEntryMeta {
    pub name: String,
    #[serde(default)]
    pub source_path: Option<String>,
    pub name_hash: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub mip_count: u32,
    pub mips: Vec<TextureMipMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMipMeta {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub byte_offset: u64,
    pub byte_len: u64,
}
