//! Deterministic engine-wide 64-bit hashing primitives.
//!
//! These small functions replace local copies scattered across gameplay,
//! procedural generation and render planning. They are intentionally stable:
//! changing their constants changes persistent IDs and deterministic worlds.

/// Combines `value` into an existing 64-bit hash seed.
#[inline]
pub const fn hash_combine_u64(mut seed: u64, value: u64) -> u64 {
    seed ^= value
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(seed << 6)
        .wrapping_add(seed >> 2);
    seed
}

/// SplitMix64 finalizer used to decorrelate deterministic integer IDs/seeds.
#[inline]
pub const fn avalanche_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Stable FNV-1a hash for authored names and deterministic content identifiers.
#[inline]
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_reference_vectors_do_not_change() {
        assert_eq!(fnv1a_64(b"NorthStar"), 0xe75b_c590_2649_33ac);
        assert_eq!(avalanche_u64(0), 0);
        assert_eq!(hash_combine_u64(0, 1), 0x9e37_79b9_7f4a_7c16);
    }
}
