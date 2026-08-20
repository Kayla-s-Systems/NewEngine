#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Coarse-grained subsystem identifiers for batching.
///
/// The legacy low bits remain supported for project-defined/coarse dependency
/// declarations alongside the named `AccessDomain` range.
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

/// Named engine-owned dependency domains used by the simulation scheduler.
///
/// Bits 0..63 remain available to legacy/coarse `Subsystem` declarations and
/// project-defined domains. Engine component/resource domains start at bit 64 so
/// migration does not collide with the historical low-bit convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum AccessDomain {
    CharacterControl = 64,
    CharacterInput = 65,
    CameraControl = 66,
    CameraInput = 67,
    CameraRig = 68,
    FollowTarget = 69,
    ControllerIntents = 70,
    Transform = 71,
    Velocity = 72,
    PhysicsState = 73,
    SceneDerived = 74,
    WorldTopology = 75,
}

impl AccessDomain {
    pub const COUNT: usize = 12;

    #[inline]
    pub const fn bit(self) -> u32 {
        self as u32
    }

    #[inline]
    pub const fn mask(self) -> u128 {
        1u128 << self.bit()
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CharacterControl => "character-control",
            Self::CharacterInput => "character-input",
            Self::CameraControl => "camera-control",
            Self::CameraInput => "camera-input",
            Self::CameraRig => "camera-rig",
            Self::FollowTarget => "follow-target",
            Self::ControllerIntents => "controller-intents",
            Self::Transform => "transform",
            Self::Velocity => "velocity",
            Self::PhysicsState => "physics-state",
            Self::SceneDerived => "scene-derived",
            Self::WorldTopology => "world-topology",
        }
    }

    #[inline]
    pub const fn all() -> [Self; Self::COUNT] {
        [
            Self::CharacterControl,
            Self::CharacterInput,
            Self::CameraControl,
            Self::CameraInput,
            Self::CameraRig,
            Self::FollowTarget,
            Self::ControllerIntents,
            Self::Transform,
            Self::Velocity,
            Self::PhysicsState,
            Self::SceneDerived,
            Self::WorldTopology,
        ]
    }
}

/// Exact bits that caused two access declarations to conflict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessConflictMask {
    /// Left writes overlapping right writes.
    pub write_write: u128,
    /// Left writes overlapping right reads.
    pub write_read: u128,
    /// Left reads overlapping right writes.
    pub read_write: u128,
}

impl AccessConflictMask {
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.write_write == 0 && self.write_read == 0 && self.read_write == 0
    }

    #[inline]
    pub const fn blocking_mask(self) -> u128 {
        self.write_write | self.write_read | self.read_write
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self {
            write_write: self.write_write | other.write_write,
            write_read: self.write_read | other.write_read,
            read_write: self.read_write | other.read_write,
        }
    }
}

/// Access declaration used by the deterministic scheduler and `engine.threading`
/// simulation executor. Masks may combine legacy project bits with named engine
/// component/resource domains.
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
    pub const fn conflict_mask(self, other: Self) -> AccessConflictMask {
        AccessConflictMask {
            write_write: self.write & other.write,
            write_read: self.write & other.read,
            read_write: self.read & other.write,
        }
    }

    #[inline]
    pub const fn conflicts(self, other: Self) -> bool {
        !self.conflict_mask(other).is_empty()
    }

    #[inline]
    pub const fn read_domain(domain: AccessDomain) -> Self {
        Self::rw(domain.mask(), 0)
    }

    #[inline]
    pub const fn write_domain(domain: AccessDomain) -> Self {
        Self::rw(0, domain.mask())
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
