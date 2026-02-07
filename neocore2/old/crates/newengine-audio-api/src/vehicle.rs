use crate::ids::AudioEntityId;
use crate::math::Vec3f;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct VehicleAudioDesc {
    pub engine_rpm: f32,
    pub speed_mps: f32,
    pub skid: f32,
    pub surface: u32,
    pub exhaust_pos: Vec3f,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait VehicleAudioV1: Send + Sync {
    fn set_vehicle_state(&self, vehicle: AudioEntityId, desc: VehicleAudioDesc);
}

#[cfg(feature = "abi")]
pub type VehicleAudioV1Dyn<'a> = VehicleAudioV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type VehicleAudioV1Dyn<'a> = &'a dyn VehicleAudioV1;