#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;

/// Stable primitive identifier.
///
/// Use `fnv1a_64("kalitech.primitive.cube.v1")` for deterministic IDs.
/// The string itself should be treated as the canonical identity; hash is a compact key.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PrimitiveId(pub u64);

impl PrimitiveId {
    #[inline]
    pub const fn new(v: u64) -> Self {
        Self(v)
    }
}

impl fmt::Debug for PrimitiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrimitiveId(0x{:016x})", self.0)
    }
}

/// Deterministic FNV-1a 64-bit hash for IDs.
///
/// Keep namespaced strings, e.g.:
/// - "kalitech.primitive.cube.v1"
/// - "kalitech.primitive.plane.v1"
#[inline]
pub const fn fnv1a_64(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut hash: u64 = 0xcbf29ce484222325; // offset basis
    let mut i = 0usize;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}