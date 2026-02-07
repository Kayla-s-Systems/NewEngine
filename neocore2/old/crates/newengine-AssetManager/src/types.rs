use crate::id::AssetId;
use std::path::{Component, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetKey {
    pub logical_path: PathBuf,
    pub settings_hash: u64,
}

impl AssetKey {
    #[inline]
    pub fn new(logical_path: impl Into<PathBuf>, settings_hash: u64) -> Self {
        let p = normalize_logical_path(logical_path.into())
            .unwrap_or_else(|_| PathBuf::from("invalid_path"));
        Self {
            logical_path: p,
            settings_hash,
        }
    }

    #[inline]
    pub fn id(&self) -> AssetId {
        AssetId::from_key(self)
    }
}

/// Marker trait for typed, CPU-side assets (optional layer).
pub trait Asset: Send + Sync + 'static {
    fn type_name() -> &'static str;
}

/// Opaque asset payload produced by importers (including plugin importers).
#[derive(Debug, Clone)]
pub struct AssetBlob {
    pub type_id: Arc<str>,
    pub format: Arc<str>,
    pub payload: Vec<u8>,
    pub meta_json: Arc<str>,
    pub dependencies: Vec<AssetDependency>,
}

#[derive(Debug, Clone)]
pub struct AssetDependency {
    pub logical_path: PathBuf,
    pub settings_hash: u64,
    pub type_hint: Arc<str>,
    pub usage: Arc<str>,
}

#[derive(Debug, Clone)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed(Arc<str>),
}

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

/// Importer priority (host-defined). Higher wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImporterPriority(pub i32);

impl ImporterPriority {
    #[inline]
    pub const fn new(v: i32) -> Self {
        Self(v)
    }
}

#[derive(Debug)]
enum NormalizePathError {
    Invalid,
}

/// Logical path contract:
/// - relative
/// - no root/prefix
/// - no '.' or '..'
/// - platform-stable via components joining
#[inline]
fn normalize_logical_path(p: PathBuf) -> Result<PathBuf, NormalizePathError> {
    let mut out = PathBuf::new();

    for c in p.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            Component::ParentDir => {
                // Reject traversal deterministically to keep AssetId stable and non-ambiguous.
                return Err(NormalizePathError::Invalid);
            }
            Component::Prefix(_) | Component::RootDir => {
                // Strip absolute prefixes for logical path stability.
                return Err(NormalizePathError::Invalid);
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err(NormalizePathError::Invalid);
    }

    Ok(out)
}