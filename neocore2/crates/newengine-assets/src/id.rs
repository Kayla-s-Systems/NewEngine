use crate::types::AssetKey;
use blake3::Hasher;
use std::hash::Hash;
use std::path::Path;

/// Stable asset identifier (content-addressed by key, not by runtime pointer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AssetId(pub(crate) u128);

impl AssetId {
    #[inline]
    pub fn to_u128(self) -> u128 {
        self.0
    }

    #[inline]
    pub fn from_key(key: &AssetKey) -> Self {
        let mut h = Hasher::new();
        hash_path(&mut h, &key.logical_path);
        h.update(&key.settings_hash.to_le_bytes());
        let out = h.finalize();
        let bytes = out.as_bytes();
        let mut lo = [0u8; 16];
        lo.copy_from_slice(&bytes[0..16]);
        Self(u128::from_le_bytes(lo))
    }
}

#[inline]
fn hash_path(h: &mut Hasher, p: &Path) {
    // We hash the normalized string representation to keep behavior stable.
    // This remains deterministic for the same logical path inside the project.
    let s = p.to_string_lossy();
    h.update(s.as_bytes());
}