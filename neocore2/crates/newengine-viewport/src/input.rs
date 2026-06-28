#![forbid(unsafe_op_in_unsafe_fn)]

/// Movement key bitmask shared between UI and render thread.
///
/// The editor publishes this mask through `ViewportBridge`.
pub const MOVE_W: u64 = 1 << 0;
pub const MOVE_A: u64 = 1 << 1;
pub const MOVE_S: u64 = 1 << 2;
pub const MOVE_D: u64 = 1 << 3;
pub const MOVE_UP: u64 = 1 << 4; // Q
pub const MOVE_DOWN: u64 = 1 << 5; // E
pub const MOVE_SHIFT: u64 = 1 << 6;
