use super::*;
use newengine_math::collections::BoundedCache;

const XML_CACHE_CAPACITY: usize = 64;
const COMPILE_CACHE_CAPACITY: usize = 96;
const DIALECT_CACHE_CAPACITY: usize = 32;

#[derive(Clone, Debug)]
pub(crate) struct CachedXmlCentral {
    pub(crate) xml: String,
    pub(crate) vfs_path: String,
}

pub struct AssetsUiRuntimeState {
    pub(crate) client: AssetServiceClient,
    pub(crate) xml_cache: BoundedCache<String, CachedXmlCentral>,
    pub(crate) compile_cache: BoundedCache<String, AssetsUiCompileResponse>,
    pub(crate) dialect_cache: BoundedCache<String, NeUiDialect>,
}

impl AssetsUiRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,
            xml_cache: BoundedCache::new(XML_CACHE_CAPACITY),
            compile_cache: BoundedCache::new(COMPILE_CACHE_CAPACITY),
            dialect_cache: BoundedCache::new(DIALECT_CACHE_CAPACITY),
        }
    }
}
