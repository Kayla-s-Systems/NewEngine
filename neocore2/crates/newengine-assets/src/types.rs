use crate::id::AssetId;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

/// Asset lookup key: (logical path + import settings hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetKey {
    pub logical_path: PathBuf,
    pub settings_hash: u64,
}

impl AssetKey {
    #[inline]
    pub fn new(logical_path: impl Into<PathBuf>, settings_hash: u64) -> Self {
        Self {
            logical_path: logical_path.into(),
            settings_hash,
        }
    }

    #[inline]
    pub fn id(&self) -> AssetId {
        AssetId::from_key(self)
    }
}

/// Strongly-typed handle used by game code/systems.
/// The handle is stable and can be stored inside ECS/resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle<T: Asset> {
    id: AssetId,
    _pd: PhantomData<fn() -> T>,
}

impl<T: Asset> Handle<T> {
    #[inline]
    pub fn id(self) -> AssetId {
        self.id
    }

    #[inline]
    pub(crate) fn new(id: AssetId) -> Self {
        Self {
            id,
            _pd: PhantomData,
        }
    }
}

/// Base asset marker trait (CPU-side).
pub trait Asset: Send + Sync + 'static {
    fn type_name() -> &'static str;
}

/// Shared reference to a loaded asset instance.
pub type AssetRef<T> = Arc<T>;

#[derive(Debug, Clone)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed(Arc<str>),
}

/// Minimal error type for import/load pipeline.
#[derive(Debug, Clone)]
pub struct AssetError {
    msg: Arc<str>,
}

impl AssetError {
    #[inline]
    pub fn new(msg: impl Into<Arc<str>>) -> Self {
        Self { msg: msg.into() }
    }

    #[inline]
    pub fn msg(&self) -> &str {
        &self.msg
    }
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for AssetError {}