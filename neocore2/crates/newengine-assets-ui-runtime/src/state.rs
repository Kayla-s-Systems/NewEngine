use super::*;

#[derive(Clone, Debug)]
pub(crate) struct CachedXmlCentral {
    pub(crate) xml: String,
    pub(crate) vfs_path: String,
}

pub struct AssetsUiRuntimeState {
    pub(crate) client: AssetServiceClient,

    pub(crate) xml_cache: HashMap<String, CachedXmlCentral>,

    pub(crate) compile_cache: HashMap<String, AssetsUiCompileResponse>,

    pub(crate) dialect_cache: HashMap<String, NeUiDialect>,
}

impl AssetsUiRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,

            xml_cache: HashMap::new(),

            compile_cache: HashMap::new(),

            dialect_cache: HashMap::new(),
        }
    }
}
