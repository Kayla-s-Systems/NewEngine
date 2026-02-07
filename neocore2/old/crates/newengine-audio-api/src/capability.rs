use bytemuck::{Pod, Zeroable};

#[cfg(feature = "abi")]
use abi_stable::StableAbi;

/// Stable service identifier for discovery/registry in the host.
/// Keep it as a constant string to avoid hardcoding plugin implementations in the ABI layer.
pub const AUDIO_SERVICE_ID_V1: &str = "newengine.audio.v1";

/// ABI/API version for the audio service surface.
/// Increment only on breaking changes.
pub const AUDIO_API_VERSION_V1: u32 = 1;

/// Capability bitmask for optional sub-APIs.
/// This stays ABI-stable and can be extended safely.
#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioCapabilityMask(pub u64);

impl AudioCapabilityMask {
    pub const NONE: Self = Self(0);

    pub const AMBIENCE: Self = Self(1 << 0);
    pub const ENVIRONMENT: Self = Self(1 << 1);
    pub const MIXER: Self = Self(1 << 2);
    pub const MUSIC: Self = Self(1 << 3);
    pub const VOICE: Self = Self(1 << 4);
    pub const VEHICLE: Self = Self(1 << 5);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
