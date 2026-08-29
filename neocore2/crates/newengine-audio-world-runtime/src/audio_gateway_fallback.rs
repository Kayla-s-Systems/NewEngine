#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RResult;
use newengine_audio_api::{
    AudioFeedbackAck, AudioFeedbackDrain, AudioFeedbackEvent, AudioServiceInfo,
    AUDIO_BACKEND_CAPABILITY_ID, AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1,
    AUDIO_SERVICE_METHOD_INVOKE, AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1, ENGINE_AUDIO_SERVICE_ID,
};
use newengine_service_kit::{
    decode_json_payload, engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

const AUDIO_EVENT_QUEUE_CAPACITY: usize = 128;
const AUDIO_GATEWAY_OWNER: &str = "newengine-audio-world-runtime.audio-gateway";

static AUDIO_GATEWAY: OnceLock<Arc<Mutex<AudioGatewayState>>> = OnceLock::new();
static AUDIO_GATEWAY_REGISTERED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Default)]
struct AudioGatewayState {
    events: VecDeque<AudioFeedbackEvent>,
}

impl AudioGatewayState {
    fn push(&mut self, event: AudioFeedbackEvent) -> AudioFeedbackAck {
        if self.events.len() >= AUDIO_EVENT_QUEUE_CAPACITY {
            let _ = self.events.pop_front();
        }
        self.events.push_back(event);
        AudioFeedbackAck {
            accepted: true,
            provider: "engine.audio.echo-event-queue".to_owned(),
            queued_events: self.events.len(),
        }
    }

    fn drain(&mut self) -> AudioFeedbackDrain {
        AudioFeedbackDrain {
            events: self.events.drain(..).collect(),
        }
    }
}

fn gateway_state() -> Arc<Mutex<AudioGatewayState>> {
    Arc::clone(AUDIO_GATEWAY.get_or_init(|| Arc::new(Mutex::new(AudioGatewayState::default()))))
}

fn audio_gateway_service(
    state: Arc<Mutex<AudioGatewayState>>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = AudioServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_AUDIO_SERVICE_ID,
        AUDIO_GATEWAY_OWNER,
        AUDIO_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_AUDIO_SERVICE_ID)
    .notes("Null/queue provider until a mixer plugin overrides engine.audio");

    JsonServiceRouter::with_shared_state(ENGINE_AUDIO_SERVICE_ID, state)
        .describe_json(&description)
        .info(AudioServiceInfo::default)
        .blob(AUDIO_SERVICE_METHOD_INVOKE, |state, payload| {
            if payload.is_empty() {
                return ok_json(AudioFeedbackAck {
                    accepted: true,
                    provider: "engine.audio.echo-event-queue".to_owned(),
                    queued_events: state.events.len(),
                });
            }
            let event = match decode_json_payload::<AudioFeedbackEvent>(
                ENGINE_AUDIO_SERVICE_ID,
                AUDIO_SERVICE_METHOD_INVOKE,
                &payload,
            ) {
                Ok(event) => event,
                Err(e) => return RResult::RErr(e),
            };
            ok_json(state.push(event))
        })
        .blob(AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1, |state, payload| {
            if payload.is_empty() {
                return ok_json(AudioFeedbackAck {
                    accepted: true,
                    provider: "engine.audio.echo-event-queue".to_owned(),
                    queued_events: state.events.len(),
                });
            }
            let event = match decode_json_payload::<AudioFeedbackEvent>(
                ENGINE_AUDIO_SERVICE_ID,
                AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
                &payload,
            ) {
                Ok(event) => event,
                Err(e) => return RResult::RErr(e),
            };
            ok_json(state.push(event))
        })
        .get_json(AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1, |state| {
            state.drain()
        })
        .blob(AUDIO_SERVICE_METHOD_SHUTDOWN_V1, |state, _payload| {
            state.events.clear();
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_audio_gateway_best_effort() {
    if AUDIO_GATEWAY_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let service = audio_gateway_service(gateway_state());
    if register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_AUDIO_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Audio,
        provider_service: ENGINE_AUDIO_SERVICE_ID,
        provider_route: "engine.audio.echo",
        capability: AUDIO_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: AUDIO_GATEWAY_OWNER,
        service,
    })
    .is_err()
    {
        AUDIO_GATEWAY_REGISTERED.store(false, Ordering::Release);
    }
}
