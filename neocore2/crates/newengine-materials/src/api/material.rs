use core::hash::{Hash, Hasher};

/// Stable, deterministic material identifier.
///
/// `0` is reserved as an invalid id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MaterialId(pub u64);

impl MaterialId {
    const INSTANCE_BIT: u64 = 1u64 << 63;

    #[inline]
    pub const fn invalid() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    #[inline]
    pub const fn is_instance(self) -> bool {
        (self.0 & Self::INSTANCE_BIT) != 0
    }

    #[inline]
    pub const fn is_asset(self) -> bool {
        self.is_valid() && !self.is_instance()
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Material reference component (entity -> registry id).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialRef {
    pub id: MaterialId,
}

impl Default for MaterialRef {
    #[inline]
    fn default() -> Self {
        Self {
            id: MaterialId::invalid(),
        }
    }
}

/// Deterministic 64-bit FNV-1a hash.
#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;

    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Create a deterministic `MaterialId` from a name.
#[inline]
pub fn material_id_from_name(name: &str) -> MaterialId {
    let mut v = fnv1a64(name.as_bytes());
    // Keep top bit clear for asset ids.
    v &= !MaterialId::INSTANCE_BIT;
    if v == 0 {
        v = 1;
    }
    MaterialId(v)
}

/// Create a deterministic instance id from a base material and instance name.
///
/// Instance ids have the top bit set to distinguish them from asset ids.
#[inline]
pub fn material_instance_id(base: MaterialId, instance_name: &str) -> MaterialId {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&base.0.to_le_bytes());
    let h1 = fnv1a64(&buf);
    let h2 = fnv1a64(instance_name.as_bytes());

    let mut v = h1 ^ h2;
    v |= MaterialId::INSTANCE_BIT;
    if v == MaterialId::INSTANCE_BIT {
        v |= 1;
    }
    MaterialId(v)
}

/// Cheaply perturb an id in a deterministic way (used for collision resolution).
#[inline]
pub fn bump_id(id: MaterialId) -> MaterialId {
    let mut v = id.0.wrapping_add(1);
    if v == 0 {
        v = 1;
    }
    MaterialId(v)
}

// Silence "unused import" in no_std-ish scenarios; keep in api for future extensions.
#[allow(dead_code)]
fn _hash_sanity<H: Hasher>(_h: &mut H) {}
