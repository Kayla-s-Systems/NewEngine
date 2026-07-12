#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Coarse-grained subsystem identifiers for batching.
///
/// The schedule uses these bits to describe conflicts for deterministic ordering
/// now, and for a future `engine.threading`-owned parallel executor later.
///
/// You are free to define your own subsystem bits in downstream code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Subsystem {
    /// Gameplay/world simulation.
    Gameplay = 0,
    /// Camera/controller simulation.
    Camera = 1,
    /// Scene derived caches/bounds.
    Scene = 2,
}

/// Access declaration used by the deterministic scheduler and future jobs executor.
///
/// This is intentionally coarse-grained and ABI-stable.
///
/// - `read`: resources read by the system
/// - `write`: resources written by the system
///
/// Conflict rule:
/// - write/write conflicts
/// - write/read conflicts
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessMask {
    pub read: u128,
    pub write: u128,
}

impl AccessMask {
    #[inline]
    pub const fn none() -> Self {
        Self { read: 0, write: 0 }
    }

    #[inline]
    pub const fn read(bit: u32) -> Self {
        Self {
            read: 1u128 << bit,
            write: 0,
        }
    }

    #[inline]
    pub const fn write(bit: u32) -> Self {
        Self {
            read: 0,
            write: 1u128 << bit,
        }
    }

    #[inline]
    pub const fn rw(read: u128, write: u128) -> Self {
        Self { read, write }
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self {
            read: self.read | other.read,
            write: self.write | other.write,
        }
    }

    #[inline]
    pub const fn conflicts(self, other: Self) -> bool {
        ((self.write & (other.write | other.read)) != 0) || ((other.write & self.read) != 0)
    }

    #[inline]
    pub const fn gameplay_rw() -> Self {
        Self::rw(0, 1u128 << (Subsystem::Gameplay as u32))
    }

    #[inline]
    pub const fn camera_rw() -> Self {
        Self::rw(0, 1u128 << (Subsystem::Camera as u32))
    }

    #[inline]
    pub const fn scene_rw() -> Self {
        Self::rw(0, 1u128 << (Subsystem::Scene as u32))
    }
}
