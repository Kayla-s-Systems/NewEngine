use crate::ids::{AudioBusId, AudioEntityId, AudioEventId, AudioSnapshotId};
use crate::types::{AudioEntityDesc, AudioListenerDesc, SpatializationDesc};

#[cfg(feature = "abi")]
use abi_stable::{sabi_trait, std_types::RBox};

#[cfg_attr(feature = "abi", sabi_trait)]
pub trait AudioSystemV1: Send + Sync {
    fn create_entity(&self, desc: AudioEntityDesc) -> AudioEntityId;
    fn destroy_entity(&self, id: AudioEntityId);
    fn set_entity_desc(&self, id: AudioEntityId, desc: AudioEntityDesc);

    fn set_listener(&self, listener: AudioListenerDesc);
    fn set_spatialization_defaults(&self, desc: SpatializationDesc);

    fn update(&self, dt_sec: f32);

    fn post_event(&self, event: AudioEventId, target: AudioEntityId) -> u64;
    fn stop_event_instance(&self, instance_id: u64);

    fn set_bus_gain(&self, bus: AudioBusId, gain: f32);
    fn set_snapshot(&self, snapshot: AudioSnapshotId, intensity: f32);
}

#[cfg(feature = "abi")]
pub type AudioSystemV1Dyn<'a> = AudioSystemV1_TO<'a, RBox<()>>;

#[cfg(not(feature = "abi"))]
pub type AudioSystemV1Dyn<'a> = &'a dyn AudioSystemV1;
