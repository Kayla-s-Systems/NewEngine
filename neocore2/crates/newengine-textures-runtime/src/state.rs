use newengine_assets::{AssetServiceClient, Rgba8TextureAsset, RuntimeTextureAsset};
use newengine_math::collections::{BoundedCache, FxHashMap};

use crate::dto::TextureRefValidation;

const MANIFEST_CACHE_CAPACITY: usize = 64;
const VALIDATION_CACHE_CAPACITY: usize = 512;
const RGBA8_PACKET_CACHE_CAPACITY: usize = 24;
const RUNTIME_DICTIONARY_CACHE_CAPACITY: usize = 8;

#[derive(Clone)]
pub struct TextureRuntimeState {
    pub(crate) client: AssetServiceClient,
    pub(crate) manifest_cache: BoundedCache<String, serde_json::Value>,
    pub(crate) validation_cache: BoundedCache<String, TextureRefValidation>,
    pub(crate) rgba8_packet_cache: BoundedCache<String, Rgba8TextureAsset>,
    pub(crate) runtime_dictionary_cache: BoundedCache<String, RuntimeTextureDictionaryCache>,
}

impl TextureRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,
            manifest_cache: BoundedCache::new(MANIFEST_CACHE_CAPACITY),
            validation_cache: BoundedCache::new(VALIDATION_CACHE_CAPACITY),
            rgba8_packet_cache: BoundedCache::new(RGBA8_PACKET_CACHE_CAPACITY),
            runtime_dictionary_cache: BoundedCache::new(RUNTIME_DICTIONARY_CACHE_CAPACITY),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeTextureDictionaryCache {
    pub(crate) entries_by_name: FxHashMap<String, RuntimeTextureAsset>,
    pub(crate) entry_hash_to_name: FxHashMap<u64, String>,
}
