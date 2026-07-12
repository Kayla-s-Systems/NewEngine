use std::collections::HashMap;

use newengine_assets::{AssetServiceClient, Rgba8TextureAsset, RuntimeTextureAsset};

use crate::dto::TextureRefValidation;

#[derive(Clone)]
pub struct TextureRuntimeState {
    pub(crate) client: AssetServiceClient,
    pub(crate) manifest_cache: HashMap<String, serde_json::Value>,
    pub(crate) validation_cache: HashMap<String, TextureRefValidation>,
    pub(crate) rgba8_packet_cache: HashMap<String, Rgba8TextureAsset>,
    pub(crate) runtime_dictionary_cache: HashMap<String, RuntimeTextureDictionaryCache>,
}

impl TextureRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,
            manifest_cache: HashMap::default(),
            validation_cache: HashMap::default(),
            rgba8_packet_cache: HashMap::default(),
            runtime_dictionary_cache: HashMap::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeTextureDictionaryCache {
    pub(crate) entries_by_name: HashMap<String, RuntimeTextureAsset>,
    pub(crate) entry_hash_to_name: HashMap<u64, String>,
}
