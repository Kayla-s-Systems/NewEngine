use crate::capability::AudioCapabilityMask;
use crate::ids::{AudioBusId, AudioEntityId, AudioEventId, AudioSnapshotId};
use crate::types::{AudioEntityDesc, AudioListenerDesc, SpatializationDesc};
use bytemuck::{Pod, Zeroable};

#[cfg(feature = "abi")]
use abi_stable::StableAbi;

/* =============================================================================================
Service ID + method names (for newengine-plugin-api::ServiceV1)
============================================================================================= */

pub mod method {
    pub const API_VERSION: &str = "audio.api_version";
    pub const CAPABILITIES: &str = "audio.capabilities";

    pub const UPDATE: &str = "audio.update";
    pub const SET_LISTENER: &str = "audio.set_listener";
    pub const SET_SPATIAL_DEFAULTS: &str = "audio.set_spatial_defaults";

    pub const CREATE_ENTITY: &str = "audio.create_entity";
    pub const DESTROY_ENTITY: &str = "audio.destroy_entity";
    pub const SET_ENTITY_DESC: &str = "audio.set_entity_desc";

    pub const POST_EVENT: &str = "audio.post_event";
    pub const STOP_EVENT_INSTANCE: &str = "audio.stop_event_instance";

    pub const SET_BUS_GAIN: &str = "audio.set_bus_gain";
    pub const SET_SNAPSHOT: &str = "audio.set_snapshot";
}

/* =============================================================================================
POD requests/responses (wire format)
============================================================================================= */

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct ApiVersionRes {
    pub api_version: u32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct CapabilitiesRes {
    pub mask: AudioCapabilityMask,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct UpdateReq {
    pub dt_sec: f32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SetListenerReq {
    pub listener: AudioListenerDesc,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SetSpatialDefaultsReq {
    pub desc: SpatializationDesc,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct CreateEntityReq {
    pub desc: AudioEntityDesc,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct CreateEntityRes {
    pub id: AudioEntityId,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct DestroyEntityReq {
    pub id: AudioEntityId,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SetEntityDescReq {
    pub id: AudioEntityId,
    pub desc: AudioEntityDesc,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct PostEventReq {
    pub event: AudioEventId,
    pub target: AudioEntityId,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct PostEventRes {
    pub instance_id: u64,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct StopEventInstanceReq {
    pub instance_id: u64,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SetBusGainReq {
    pub bus: AudioBusId,
    pub gain: f32,
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct SetSnapshotReq {
    pub snapshot: AudioSnapshotId,
    pub intensity: f32,
}
