use crate::types::{Asset, AssetError, AssetKey};
use std::any::{TypeId, Any};
use std::sync::Arc;

/// Typed importer: bytes -> T.
/// Keeping it typed avoids any unsafe downcasting in the store.
pub trait Importer<T: Asset>: Send + Sync + 'static {
    fn supported_extensions(&self) -> &'static [&'static str];

    fn import(&self, bytes: &[u8], key: &AssetKey) -> Result<T, AssetError>;
}

/// Type-erased wrapper so store can keep heterogeneous importers.
pub(crate) trait AnyImporter: Send + Sync + 'static {
    fn output_type(&self) -> TypeId;
    fn supported_extensions(&self) -> &'static [&'static str];
    fn import_dyn(&self, bytes: &[u8], key: &AssetKey) -> Result<Arc<dyn Any + Send + Sync>, AssetError>;
}

pub(crate) struct ImporterBox<T: Asset> {
    inner: Box<dyn Importer<T>>,
}

impl<T: Asset> ImporterBox<T> {
    #[inline]
    pub fn new(inner: Box<dyn Importer<T>>) -> Self {
        Self { inner }
    }
}

impl<T: Asset> AnyImporter for ImporterBox<T> {
    #[inline]
    fn output_type(&self) -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn supported_extensions(&self) -> &'static [&'static str] {
        self.inner.supported_extensions()
    }

    fn import_dyn(
        &self,
        bytes: &[u8],
        key: &AssetKey,
    ) -> Result<Arc<dyn Any + Send + Sync>, AssetError> {
        let v = self.inner.import(bytes, key)?;
        Ok(Arc::new(v) as Arc<dyn Any + Send + Sync>)
    }
}