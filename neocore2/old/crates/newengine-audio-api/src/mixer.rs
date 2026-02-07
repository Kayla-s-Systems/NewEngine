use crate::ids::AudioBusId;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct DuckingDesc {
    pub bus: AudioBusId,
    pub amount: f32,
    pub attack_sec: f32,
    pub release_sec: f32,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait MixerSystemV1: Send + Sync {
    fn set_master_gain(&self, gain: f32);
    fn set_ducking(&self, desc: DuckingDesc);
}

#[cfg(feature = "abi")]
pub type MixerSystemV1Dyn<'a> = MixerSystemV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type MixerSystemV1Dyn<'a> = &'a dyn MixerSystemV1;