#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral client calls for the stable `engine.audio` gateway.

use newengine_audio_api::{
    AudioRouteGainAck, AudioRouteGainRequest, AudioCuePlayRequest, AudioCuePreloadRequest,
    AudioDiagnostics, AudioFeedbackEvent, AudioFeedbackKind, AudioListenerState, AudioPlayAck,
    AudioPlayRequest, AudioPreloadAck, AudioPreloadRequest, AudioRenderClock, AudioServiceInfo,
    AudioStopVoiceRequest, AudioStreamPlayRequest, AudioVoiceAck, AudioVoiceBudgetAck,
    AudioVoiceBudgetConfig, AudioVoiceRenderScheduleAck, AudioVoiceRenderScheduleRequest,
    AudioVoiceUpdateRequest, AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, AUDIO_SERVICE_METHOD_INFO,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1, AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1, AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1, AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1, ENGINE_AUDIO_SERVICE_ID,
};

pub fn emit_audio_feedback(kind: AudioFeedbackKind, frame_index: u64) {
    let event = AudioFeedbackEvent::ui(kind, frame_index);
    let payload = match serde_json::to_vec(&event) {
        Ok(payload) => payload,
        Err(_) => return,
    };
    let _ = newengine_core::call_service_v1_optional(
        ENGINE_AUDIO_SERVICE_ID,
        AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
        &payload,
    );
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

pub fn audio_service_info() -> Result<Option<AudioServiceInfo>, String> {
    call_audio_get_json(AUDIO_SERVICE_METHOD_INFO)
}

pub fn audio_playback_available() -> Result<bool, String> {
    Ok(audio_service_info()?.is_some_and(|info| info.supports_playback()))
}

pub fn preload_audio_clip(
    request: &AudioPreloadRequest,
) -> Result<Option<AudioPreloadAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1, request)
}

pub fn preload_audio_cue(
    request: &AudioCuePreloadRequest,
) -> Result<Option<AudioPreloadAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1, request)
}

pub fn play_audio_cue(request: &AudioCuePlayRequest) -> Result<Option<AudioPlayAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1, request)
}

pub fn play_audio_clip(request: &AudioPlayRequest) -> Result<Option<AudioPlayAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1, request)
}

pub fn play_audio_stream(request: &AudioStreamPlayRequest) -> Result<Option<AudioPlayAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1, request)
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

pub fn set_audio_route_gain(
    request: &AudioRouteGainRequest,
) -> Result<Option<AudioRouteGainAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1, request)
}

pub fn set_audio_voice_budgets(
    request: &AudioVoiceBudgetConfig,
) -> Result<Option<AudioVoiceBudgetAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1, request)
}

/// Converts the canonical resolved camera frame into the provider-neutral audio listener pose.
pub fn audio_listener_from_camera_snapshot(
    snapshot: &newengine_camera_contracts::CameraFrameSnapshot,
) -> Option<AudioListenerState> {
    if !snapshot.finite
        || !snapshot.position_ws.iter().all(|value| value.is_finite())
        || !snapshot.forward_ws.iter().all(|value| value.is_finite())
        || !snapshot.up_ws.iter().all(|value| value.is_finite())
    {
        return None;
    }
    Some(
        AudioListenerState {
            position: snapshot.position_ws,
            forward: snapshot.forward_ws,
            up: snapshot.up_ws,
            ..AudioListenerState::default()
        }
        .sanitized(),
    )
}

pub fn sync_audio_listener_from_camera_snapshot(
    snapshot: &newengine_camera_contracts::CameraFrameSnapshot,
) {
    let Some(listener) = audio_listener_from_camera_snapshot(snapshot) else {
        return;
    };
    if let Err(error) = set_audio_listener(&listener) {
        newengine_ulog_api::ulog::trace!("audio listener sync skipped provider_error='{}'", error);
    }
}

pub fn audio_render_clock() -> Result<Option<AudioRenderClock>, String> {
    call_audio_get_json(AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1)
}

pub fn schedule_audio_voice_render(
    request: &AudioVoiceRenderScheduleRequest,
) -> Result<Option<AudioVoiceRenderScheduleAck>, String> {
    call_audio_json(AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1, request)
}

pub fn audio_diagnostics() -> Result<Option<AudioDiagnostics>, String> {
    call_audio_get_json(AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_snapshot_maps_to_audio_listener() {
        let snapshot = newengine_camera_contracts::CameraFrameSnapshot {
            position_ws: [3.0, 4.0, 5.0],
            forward_ws: [0.0, 0.0, -1.0],
            up_ws: [0.0, 1.0, 0.0],
            finite: true,
            ..Default::default()
        };
        let listener = audio_listener_from_camera_snapshot(&snapshot).expect("listener");
        assert_eq!(listener.position, [3.0, 4.0, 5.0]);
        assert_eq!(listener.forward, [0.0, 0.0, -1.0]);
        assert_eq!(listener.up, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn non_finite_camera_snapshot_is_not_published() {
        let snapshot = newengine_camera_contracts::CameraFrameSnapshot {
            position_ws: [f32::NAN, 0.0, 0.0],
            finite: true,
            ..Default::default()
        };
        assert!(audio_listener_from_camera_snapshot(&snapshot).is_none());
    }
}
