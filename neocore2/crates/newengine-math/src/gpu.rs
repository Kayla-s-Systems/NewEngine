// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::Mat4;

/// Converts a matrix into a little-endian column-major byte array suitable for GPU uniform uploads.
///
/// This centralizes matrix packing to avoid layout drift across engine modules.
#[inline]
pub fn mat4_to_cols_bytes(m: Mat4) -> [u8; 64] {
    let cols: [f32; 16] = m.to_cols_array();
    let mut out = [0u8; 64];

    // f32 -> LE bytes, deterministic.
    let mut i = 0usize;
    while i < 16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&cols[i].to_le_bytes());
        i += 1;
    }

    out
}