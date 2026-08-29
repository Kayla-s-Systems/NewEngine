// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

/// Euler rotation order.
///
/// This is intentionally API-compatible with the commonly used subset of `glam::EulerRot`.
///
/// Convention:
/// - `EulerRot::ABC` means the rotation is constructed as `R = Ra(a) * Rb(b) * Rc(c)`.
/// - The angles passed to `Quat::from_euler(order, a, b, c)` correspond to the axes in the name.
///
/// Only the 6 **Tait–Bryan** orders (all axes different) are provided. This covers all
/// practical engine/editor use-cases (yaw/pitch/roll and permutations) while avoiding
/// the ambiguity and extra edge-cases of proper Euler orders (repeated axes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum EulerRot {
    /// X, then Y, then Z.
    XYZ,
    /// X, then Z, then Y.
    XZY,
    /// Yaw (Y), Pitch (X), Roll (Z).
    YXZ,
    /// Y, then Z, then X.
    YZX,
    /// Z, then X, then Y.
    ZXY,
    /// Z, then Y, then X.
    ZYX,
}
