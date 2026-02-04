#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString, RVec};

use newengine_audio_api::capability::{
    AudioCapabilityMask, AUDIO_API_VERSION_V1, AUDIO_SERVICE_ID_V1,
};
use newengine_audio_api::protocol::{
    method, ApiVersionRes, CapabilitiesRes, CreateEntityReq, CreateEntityRes, DestroyEntityReq,
    PostEventReq, PostEventRes, SetBusGainReq, SetEntityDescReq, SetListenerReq, SetSnapshotReq,
    SetSpatialDefaultsReq, StopEventInstanceReq, UpdateReq,
};
use newengine_audio_api::wire::{decode_pod, encode_pod};
use newengine_plugin_api::{Blob, HostApiV1, MethodName, ServiceV1};

pub(crate) struct AudioNullService {
    host: HostApiV1,
    next_entity: u64,
}

impl AudioNullService {
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self {
            host,
            next_entity: 1,
        }
    }

    #[inline]
    fn err(&self, msg: &'static str) -> RResult<Blob, RString> {
        (self.host.log_warn)(RString::from(msg));
        RResult::RErr(RString::from(msg))
    }

    #[inline]
    fn ok_empty(&self) -> RResult<Blob, RString> {
        RResult::ROk(RVec::new())
    }

    #[inline]
    fn ok_pod<T: bytemuck::Pod>(&self, v: &T) -> RResult<Blob, RString> {
        RResult::ROk(RVec::from(encode_pod(v)))
    }
}

impl ServiceV1 for AudioNullService {
    fn id(&self) -> RString {
        RString::from(AUDIO_SERVICE_ID_V1)
    }

    fn describe(&self) -> RString {
        RString::from(
            "NewEngine Audio service (null backend). Implements protocol-only, produces no sound.",
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let m = method.as_str();

        match m {
            method::API_VERSION => {
                let res = ApiVersionRes {
                    api_version: AUDIO_API_VERSION_V1,
                };
                return self.ok_pod(&res);
            }

            method::CAPABILITIES => {
                let res = CapabilitiesRes {
                    mask: AudioCapabilityMask::NONE,
                };
                return self.ok_pod(&res);
            }

            method::UPDATE => {
                let _req: UpdateReq = match decode_pod::<UpdateReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.update: bad payload"),
                };
                return self.ok_empty();
            }

            method::SET_LISTENER => {
                let _req: SetListenerReq = match decode_pod::<SetListenerReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.set_listener: bad payload"),
                };
                return self.ok_empty();
            }

            method::SET_SPATIAL_DEFAULTS => {
                let _req: SetSpatialDefaultsReq =
                    match decode_pod::<SetSpatialDefaultsReq>(&payload) {
                        Ok(v) => v,
                        Err(_) => return self.err("audio.set_spatial_defaults: bad payload"),
                    };
                return self.ok_empty();
            }

            method::CREATE_ENTITY => {
                let _req: CreateEntityReq = match decode_pod::<CreateEntityReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.create_entity: bad payload"),
                };

                // Note: This is a null backend. We still return stable ids for callers.
                // We must not mutate self here (ServiceV1::call takes &self).
                // So we generate a deterministic-ish id based on monotonic time if needed.
                // For now, use a hash-like approach on pointer value + payload length (stable enough per run).
                let raw = (payload.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                let id = newengine_audio_api::ids::AudioEntityId(raw | 1);

                let res = CreateEntityRes { id };
                return self.ok_pod(&res);
            }

            method::DESTROY_ENTITY => {
                let _req: DestroyEntityReq = match decode_pod::<DestroyEntityReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.destroy_entity: bad payload"),
                };
                return self.ok_empty();
            }

            method::SET_ENTITY_DESC => {
                let _req: SetEntityDescReq = match decode_pod::<SetEntityDescReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.set_entity_desc: bad payload"),
                };
                return self.ok_empty();
            }

            method::POST_EVENT => {
                let _req: PostEventReq = match decode_pod::<PostEventReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.post_event: bad payload"),
                };

                let res = PostEventRes { instance_id: 0 };
                return self.ok_pod(&res);
            }

            method::STOP_EVENT_INSTANCE => {
                let _req: StopEventInstanceReq = match decode_pod::<StopEventInstanceReq>(&payload)
                {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.stop_event_instance: bad payload"),
                };
                return self.ok_empty();
            }

            method::SET_BUS_GAIN => {
                let _req: SetBusGainReq = match decode_pod::<SetBusGainReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.set_bus_gain: bad payload"),
                };
                return self.ok_empty();
            }

            method::SET_SNAPSHOT => {
                let _req: SetSnapshotReq = match decode_pod::<SetSnapshotReq>(&payload) {
                    Ok(v) => v,
                    Err(_) => return self.err("audio.set_snapshot: bad payload"),
                };
                return self.ok_empty();
            }

            _ => {
                return self.err("audio: unknown method");
            }
        }
    }
}
