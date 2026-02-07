use crate::ids::AudioEntityId;

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox, std_types::RString, StableAbi};

#[cfg(not(feature = "abi"))]
use std::string::String;

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoicePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for VoicePriority {
    fn default() -> Self {
        Self::Normal
    }
}

#[cfg(feature = "abi")]
pub type VoiceLineKey = RString;

#[cfg(not(feature = "abi"))]
pub type VoiceLineKey = String;

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceLineDesc {
    pub key: VoiceLineKey,
    pub priority: VoicePriority,
}

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait VoiceSystemV1: Send + Sync {
    fn play_voice_line(&self, speaker: AudioEntityId, line: VoiceLineDesc) -> u64;
    fn stop_voice_instance(&self, instance_id: u64);
}

#[cfg(feature = "abi")]
pub type VoiceSystemV1Dyn<'a> = VoiceSystemV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type VoiceSystemV1Dyn<'a> = &'a dyn VoiceSystemV1;