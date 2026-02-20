// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

/// Euler rotation order.
///
/// This is intentionally API-compatible with the subset of `glam::EulerRot` used by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum EulerRot {
    /// Yaw (Y), Pitch (X), Roll (Z).
    YXZ,
}
