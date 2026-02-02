use blake3::Hasher;
use std::path::Path;

use crate::types::AssetKey;

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
        hash_logical_path(&mut h, &key.logical_path);
        h.update(&key.settings_hash.to_le_bytes());

        let out = h.finalize();
        let bytes = out.as_bytes();
        let mut lo = [0u8; 16];
        lo.copy_from_slice(&bytes[0..16]);
        Self(u128::from_le_bytes(lo))
    }
}

#[inline]
fn hash_logical_path(h: &mut Hasher, p: &Path) {
    // Stable across platforms:
    // - components joined with '/'
    // - ascii-lower for deterministic extension/path matching on Windows-like FS
    //   (logical paths should be authored consistently anyway)
    let mut first = true;
    for comp in p.components() {
        let s = comp.as_os_str().to_string_lossy();
        if s.is_empty() {
            continue;
        }
        if !first {
            h.update(b"/");
        }
        first = false;
        // Avoid locale issues; keep simple deterministic normalization
        let lower = s.as_bytes().iter().map(|b| b.to_ascii_lowercase()).collect::<Vec<u8>>();
        h.update(&lower);
    }
}