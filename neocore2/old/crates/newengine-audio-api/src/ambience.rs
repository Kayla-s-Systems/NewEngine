use crate::math::Vec3f;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct AmbienceZoneDesc {
    pub center: Vec3f,
    pub extents: Vec3f,
    pub wetness: f32,
    pub wind: f32,
    pub intensity: f32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct DirectionalAmbienceDesc {
    pub direction: Vec3f,
    pub intensity: f32,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait AmbienceSystemV1: Send + Sync {
    fn set_zone(&self, zone: AmbienceZoneDesc);
    fn set_directional(&self, desc: DirectionalAmbienceDesc);
}

#[cfg(feature = "abi")]
pub type AmbienceSystemV1Dyn<'a> = AmbienceSystemV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type AmbienceSystemV1Dyn<'a> = &'a dyn AmbienceSystemV1;