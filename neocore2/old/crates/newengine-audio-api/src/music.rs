use crate::ids::AudioTagId;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, StableAbi};

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct MusicStateDesc {
    pub intensity: f32,
    pub tension: f32,
    pub tag: AudioTagId,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait MusicSystemV1: Send + Sync {
    fn set_state(&self, state: MusicStateDesc);
    fn stop_all(&self, fade_out_sec: f32);
}

#[cfg(feature = "abi")]
pub type MusicSystemV1Dyn<'a> = MusicSystemV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type MusicSystemV1Dyn<'a> = &'a dyn MusicSystemV1;