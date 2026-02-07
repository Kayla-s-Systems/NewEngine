use crate::math::Vec3f;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct ReverbZoneDesc {
    pub center: Vec3f,
    pub extents: Vec3f,
    pub wetness: f32,
    pub room_size: f32,
    pub decay_time: f32,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait AudioEnvironmentV1: Send + Sync {
    fn set_reverb_zone(&self, zone: ReverbZoneDesc);
}

#[cfg(feature = "abi")]
pub type AudioEnvironmentV1Dyn<'a> = AudioEnvironmentV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type AudioEnvironmentV1Dyn<'a> = &'a dyn AudioEnvironmentV1;