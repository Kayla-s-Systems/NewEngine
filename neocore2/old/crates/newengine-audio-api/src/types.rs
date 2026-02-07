use bytemuck::{Pod, Zeroable};

#[cfg(feature = "abi")]
use abi_stable::StableAbi;

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SpatializationDesc {
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
    pub doppler: f32,
    // bool лучше избегать в ABI/wire: используем u32 flags
    pub flags: u32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct AudioEntityDesc {
    pub gain: f32,
    pub pitch: f32,
    pub flags: u32,
    pub bus: crate::ids::AudioBusId,
    pub pos: crate::math::Vec3f,
    pub vel: crate::math::Vec3f,
}