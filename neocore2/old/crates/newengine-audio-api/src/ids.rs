use bytemuck::{Pod, Zeroable};

#[cfg(feature = "abi")]
use abi_stable::StableAbi;

#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioEntityId(pub u64);

impl AudioEntityId {
    #[inline]
    pub const fn invalid() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioBusId(pub u32);

#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioEventId(pub u32);

#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioSnapshotId(pub u32);

#[repr(transparent)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Zeroable, Pod)]
pub struct AudioTagId(pub u32);