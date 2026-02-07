use crate::math::Vec3f;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct OcclusionRayDesc {
    pub origin: Vec3f,
    pub direction: Vec3f,
    pub max_distance: f32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct OcclusionResult {
    pub occlusion: f32,
    pub obstruction: f32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct AudioPortalDesc {
    pub id: u32,
    pub position: Vec3f,
    pub normal: Vec3f,
    pub width: f32,
    pub height: f32,
    pub openness: f32,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait AudioOcclusionV1: Send + Sync {
    fn submit_occlusion_result(&self, ray: OcclusionRayDesc, result: OcclusionResult);
    fn set_portal(&self, portal: AudioPortalDesc);
}

#[cfg(feature = "abi")]
pub type AudioOcclusionV1Dyn<'a> = AudioOcclusionV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type AudioOcclusionV1Dyn<'a> = &'a dyn AudioOcclusionV1;
