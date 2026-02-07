use crate::types::{Asset, AssetError, AssetKey};

pub trait Importer<T: Asset>: Send + Sync + 'static {
    fn supported_extensions(&self) -> &'static [&'static str];

    fn import(&self, bytes: &[u8], key: &AssetKey) -> Result<T, AssetError>;
}