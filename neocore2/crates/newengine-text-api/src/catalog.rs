use serde::{Deserialize, Serialize};

use crate::TextFormatArguments;

/// Stable 32-bit text-key hash for label lookup. The normalization is ASCII
/// case-insensitive so authored labels remain portable across providers.
pub fn stable_text_key_hash(label: &str) -> u32 {
    let mut hash = 0u32;
    for byte in label.trim().bytes() {
        hash = hash.wrapping_add(u32::from(byte.to_ascii_lowercase()));
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
    }
    hash = hash.wrapping_add(hash << 3);
    hash ^= hash >> 11;
    hash.wrapping_add(hash << 15)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextLookupKey {
    Label(String),
    Hash(u32),
}

impl TextLookupKey {
    pub fn from_label(label: impl Into<String>) -> Self {
        Self::Label(label.into())
    }

    #[inline]
    pub fn hash(&self) -> u32 {
        match self {
            Self::Label(label) => stable_text_key_hash(label),
            Self::Hash(hash) => *hash,
        }
    }
}

impl Default for TextLookupKey {
    fn default() -> Self {
        Self::Label(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextCatalogSourceKind {
    #[default]
    Core,
    Additional,
    Global,
    Patch,
    DownloadableContent,
    DebugOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextTextBlockLocation {
    #[default]
    Core,
    PatchedCore,
    Additional,
    ExtraContent,
    Debug,
}

/// Provider-neutral chunk descriptor corresponding to a four-byte chunk header.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextChunkDescriptor {
    pub kind: String,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextCatalogBlockDescriptor {
    pub id: String,
    pub locale: String,
    pub source: TextCatalogSourceKind,
    pub location: TextTextBlockLocation,
    pub priority: i32,
    pub loaded: bool,
    pub entry_count: usize,
    pub content_hash: String,
    pub chunks: Vec<TextChunkDescriptor>,
}

impl Default for TextCatalogBlockDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            locale: "und".to_owned(),
            source: TextCatalogSourceKind::Core,
            location: TextTextBlockLocation::Core,
            priority: 0,
            loaded: false,
            entry_count: 0,
            content_hash: String::new(),
            chunks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextCatalogEntry {
    pub key: String,
    pub key_hash: u32,
    pub value: String,
    pub block_id: String,
    pub locale: String,
    pub voice_name_hash: u32,
    pub mission_name_hash: u32,
}

impl Default for TextCatalogEntry {
    fn default() -> Self {
        Self {
            key: String::new(),
            key_hash: 0,
            value: String::new(),
            block_id: String::new(),
            locale: "und".to_owned(),
            voice_name_hash: 0,
            mission_name_hash: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextCatalogManifestRequest {
    pub locale: String,
    pub include_entries: bool,
    pub include_unloaded_blocks: bool,
}

impl Default for TextCatalogManifestRequest {
    fn default() -> Self {
        Self {
            locale: "und".to_owned(),
            include_entries: false,
            include_unloaded_blocks: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextCatalogManifestResponse {
    pub version: u32,
    pub active_locale: String,
    pub fallback_locales: Vec<String>,
    pub blocks: Vec<TextCatalogBlockDescriptor>,
    pub entries: Vec<TextCatalogEntry>,
    pub diagnostics: Vec<String>,
}

impl Default for TextCatalogManifestResponse {
    fn default() -> Self {
        Self {
            version: 1,
            active_locale: "und".to_owned(),
            fallback_locales: Vec::new(),
            blocks: Vec::new(),
            entries: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLocalizeRequest {
    pub key: TextLookupKey,
    pub locale: String,
    pub fallback_locales: Vec<String>,
    pub arguments: TextFormatArguments,
    pub allow_debug_override: bool,
    pub format_tokens: bool,
}

impl Default for TextLocalizeRequest {
    fn default() -> Self {
        Self {
            key: TextLookupKey::default(),
            locale: "und".to_owned(),
            fallback_locales: Vec::new(),
            arguments: TextFormatArguments::default(),
            allow_debug_override: true,
            format_tokens: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLocalizeResponse {
    pub version: u32,
    pub found: bool,
    pub text: String,
    pub resolved_key: String,
    pub key_hash: u32,
    pub locale: String,
    pub block_id: String,
    pub source: TextCatalogSourceKind,
    pub formatted: bool,
    pub diagnostics: Vec<String>,
}

impl Default for TextLocalizeResponse {
    fn default() -> Self {
        Self {
            version: 1,
            found: false,
            text: String::new(),
            resolved_key: String::new(),
            key_hash: 0,
            locale: "und".to_owned(),
            block_id: String::new(),
            source: TextCatalogSourceKind::Core,
            formatted: false,
            diagnostics: Vec::new(),
        }
    }
}
