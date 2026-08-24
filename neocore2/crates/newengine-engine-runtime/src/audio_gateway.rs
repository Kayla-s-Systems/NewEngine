#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RResult;
use newengine_audio_api::{
    AudioBusGainAck, AudioBusGainRequest, AudioDiagnostics, AudioFeedbackAck, AudioFeedbackDrain,
    AudioFeedbackEvent, AudioFeedbackKind, AudioListenerState, AudioPlayAck, AudioPlayRequest,
    AudioPreloadAck, AudioPreloadRequest, AudioServiceInfo, AudioStopVoiceRequest, AudioVoiceAck,
    AudioVoiceUpdateRequest, AUDIO_BACKEND_CAPABILITY_ID, AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1,
    AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1, AUDIO_SERVICE_METHOD_INFO,
    AUDIO_SERVICE_METHOD_INVOKE, AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1, AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1, AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1, AUDIO_SERVICE_METHOD_SHUTDOWN_V1,
    AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1, ENGINE_AUDIO_SERVICE_ID,
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
const AUDIO_GATEWAY_OWNER: &str = "newengine-engine-runtime.audio-gateway";

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
    // This bootstrap route is registered from reusable runtime construction,
    // before gateway diagnostics or routed logging may be safe to use. Do not
    // call `has_engine_gateway_route(engine.audio)` here: resolving the route can
    // re-enter the same bootstrap path in early app startup. A local atomic guard
    // is enough because this built-in queue provider is process-local and can be
    // shadowed later by normal gateway priority rules.
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

pub fn emit_audio_feedback(kind: AudioFeedbackKind, frame_index: u64) {
    let event = AudioFeedbackEvent::ui(kind, frame_index);
    let payload = match serde_json::to_vec(&event) {
        Ok(payload) => payload,
        Err(_e) => {
            return;
        }
    };

    match newengine_core::call_service_v1_optional(
        ENGINE_AUDIO_SERVICE_ID,
        AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
        &payload,
    ) {
        Ok(Some(_)) | Ok(None) => {}
        Err(_e) => {}
    }
}

fn call_audio_json<I, O>(method: &str, request: &I) -> Result<Option<O>, String>
where
    I: serde::Serialize,
    O: serde::de::DeserializeOwned,
{
    let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    let Some(bytes) =
        newengine_core::call_service_v1_optional(ENGINE_AUDIO_SERVICE_ID, method, &payload)?
    else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("engine.audio method '{method}' returned invalid JSON: {error}"))
}

fn call_audio_get_json<O>(method: &str) -> Result<Option<O>, String>
where
    O: serde::de::DeserializeOwned,
{
    let Some(bytes) =
        newengine_core::call_service_v1_optional(ENGINE_AUDIO_SERVICE_ID, method, &[])?
    else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("engine.audio method '{method}' returned invalid JSON: {error}"))
}

/// Returns the active audio provider contract exposed by `engine.audio`.
pub fn audio_service_info() -> Result<Option<AudioServiceInfo>, String> {
    call_audio_get_json(AUDIO_SERVICE_METHOD_INFO)
}

/// True only when the active provider exposes the complete playback-v1 surface.
pub fn audio_playback_available() -> Result<bool, String> {
    Ok(audio_service_info()?.is_some_and(|info| info.supports_playback()))
}

/// Preloads an authored clip into the active playback provider cache.
pub fn preload_audio_clip(
    request: &AudioPreloadRequest,
) -> Result<Option<AudioPreloadAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1, request)
}

/// Starts a 2D or spatial voice. `voice_id` in the acknowledgement is the stable
/// handle for subsequent stop/update calls.
pub fn play_audio_clip(request: &AudioPlayRequest) -> Result<Option<AudioPlayAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1, request)
}

pub fn stop_audio_voice(voice_id: u64) -> Result<Option<AudioVoiceAck>, String> {
    call_audio_json(
        AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
        &AudioStopVoiceRequest { voice_id },
    )
}

pub fn update_audio_voice(
    request: &AudioVoiceUpdateRequest,
) -> Result<Option<AudioVoiceAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1, request)
}

pub fn set_audio_listener(
    listener: &AudioListenerState,
) -> Result<Option<AudioListenerState>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1, listener)
}

pub fn set_audio_bus_gain(
    request: &AudioBusGainRequest,
) -> Result<Option<AudioBusGainAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1, request)
}

pub fn audio_diagnostics() -> Result<Option<AudioDiagnostics>, String> {
    call_audio_get_json(AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1)
}
