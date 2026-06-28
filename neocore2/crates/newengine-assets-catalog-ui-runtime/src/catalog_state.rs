use super::*;

#[derive(Clone)]
pub struct AssetsCatalogRuntimeState {
    pub(crate) client: AssetServiceClient,
}

impl AssetsCatalogRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self { client }
    }
}
