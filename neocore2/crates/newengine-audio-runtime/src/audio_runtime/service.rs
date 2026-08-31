fn audio_service(state: AudioRuntimeState) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    newengine_ulog_api::ulog::info!("audio service build: step='info-begin'");
    let info = AudioServiceInfo::playback_provider(NATIVE_AUDIO_PROVIDER_ROUTE);
    newengine_ulog_api::ulog::info!(
        "audio service build: step='info-done' methods={}",
        info.methods.len()
    );
    newengine_ulog_api::ulog::info!("audio service build: step='description-begin'");
    let description = engine_gateway_provider_service_description(
        NATIVE_AUDIO_SERVICE_ID,
        NATIVE_AUDIO_PROVIDER_ROUTE,
        AUDIO_BACKEND_CAPABILITY_ID,
        info.methods.iter().map(String::as_str),
    )
    .gateway(ENGINE_AUDIO_SERVICE_ID)
    .protocol(info.protocol.clone())
    .provider_abi(AUDIO_PROVIDER_ABI_ID)
    .features([
        "native-output",
        "rodio-cpal",
        "wav",
        "mp3",
        "vorbis",
        "flac",
        "2d-voices",
        "spatial-voices",
        "opaque-audio-routes",
        "clip-cache",
        "voice-budget",
        "voice-policy-v2",
        "reserved-voice-budgets",
        "voice-virtualization",
        "stream-logical-virtualization",
        "block-native-render-graph",
        "sample-addressed-render-scheduling",
        "yscd-sound-graph-v1",
        "sound-graph-trigger-parameters",
        "block-based-native-render-graph",
        "single-master-output",
        "sample-accurate-render-scheduling",
        "authored-attenuation",
        "physics-acoustic-state",
        "occlusion-aware-arbitration",
        "streaming-playback",
        "compressed-range-streaming",
        "seekable-streaming",
        "bounded-compressed-cache",
        "bounded-pcm-ring",
        "long-form-audio",
        "environment-zones",
        "portal-sends",
        "dynamic-reverb",
    ])
    .notes("First-party native audio provider; replaceable through engine.audio gateway routing.");
    newengine_ulog_api::ulog::info!("audio service build: step='description-done'");
    newengine_ulog_api::ulog::info!("audio service build: step='router-begin'");

    let service = JsonServiceRouter::with_state(NATIVE_AUDIO_SERVICE_ID, state)
        .describe_json(&description)
        .info(move || info.clone())
        .post_json(
            AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
            |state, event: AudioFeedbackEvent| state.play_feedback(event),
        )
        .blob(AUDIO_SERVICE_METHOD_INVOKE, |state, payload| {
            let event = match serde_json::from_slice::<AudioFeedbackEvent>(payload.as_slice()) {
                Ok(event) => event,
                Err(error) => return RResult::RErr(RString::from(error.to_string())),
            };
            ok_json(state.play_feedback(event))
        })
        .post_json_result(
            AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
            |state, request: AudioPreloadRequest| state.preload(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
            |state, request: AudioCuePreloadRequest| state.preload_cue(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
            |state, request: AudioCuePlayRequest| state.play_cue(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
            |state, request: AudioPlayRequest| state.play_clip(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
            |state, request: AudioStreamPlayRequest| state.play_stream(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
            |state, request: AudioStopVoiceRequest| state.stop_voice(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
            |state, request: AudioVoiceUpdateRequest| state.update_voice(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
            |state, listener: AudioListenerState| state.set_listener(listener),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1,
            |state, request: AudioRouteGainRequest| state.set_route_gain(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1,
            |state, request: AudioVoiceBudgetConfig| state.set_voice_budgets(request),
        )
        .get_json(AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1, |state| state.render_clock())
        .post_json(
            AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1,
            |state, request: AudioVoiceRenderScheduleRequest| state.schedule_voice_render(request),
        )
        .get_json(AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, |state| {
            state.diagnostics()
        })
        .blob(AUDIO_SERVICE_METHOD_SHUTDOWN_V1, |state, _payload: Blob| {
            state.shutdown();
            ok_empty_blob()
        })
        .into_service_v1();
    newengine_ulog_api::ulog::info!("audio service build: step='router-done'");
    service
}

/// Registers the first-party native provider when an OS audio output is usable.
/// Failure is non-fatal: the semantic queue route remains active for headless,
/// servers, CI, and machines without a sound device.
/// Builds the native audio provider service without mutating the engine gateway registry.
///
/// First-party audio plugins own service registration through `HostApiV1::register_service_v1`.
/// Keeping construction separate from registration prevents startup-FSM re-entrancy and makes
/// the native backend replaceable like render/physics/input providers.
pub fn native_audio_provider_service(
    assets: AssetServiceClient,
) -> Result<newengine_plugin_api::ServiceV1Dyn<'static>, String> {
    newengine_ulog_api::ulog::info!("audio service factory: step='state-begin'");
    let state = AudioRuntimeState::open_default(assets)?;
    newengine_ulog_api::ulog::info!("audio service factory: step='state-done'");
    let service = audio_service(state);
    newengine_ulog_api::ulog::info!("audio service factory: step='service-done'");
    Ok(service)
}

pub fn register_native_audio_provider_best_effort(assets: AssetServiceClient) -> bool {
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='enter'");
    if audio_disabled_by_env() || headless_runtime() {
        newengine_ulog_api::ulog::info!(
            "audio provider skipped route='{}' reason='{}'",
            NATIVE_AUDIO_PROVIDER_ROUTE,
            if headless_runtime() {
                "headless"
            } else {
                "disabled-by-env"
            }
        );
        return false;
    }

    // Registration ownership is HostContext-scoped. A process-global one-shot guard would
    // survive transaction rollback and would incorrectly couple multiple Engine instances.
    if newengine_plugin_host::has_service(NATIVE_AUDIO_SERVICE_ID) {
        newengine_ulog_api::ulog::info!("audio provider bootstrap: step='already-registered'");
        return true;
    }

    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='state-create-begin'");
    let state = match AudioRuntimeState::open_default(assets) {
        Ok(state) => state,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "audio provider unavailable route='{}' err='{}'; engine.audio fallback remains active",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            return false;
        }
    };

    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='state-create-done'");
    let service = audio_service(state);
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='service-build-done'");
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='gateway-register-begin'");
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_AUDIO_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Audio,
        provider_service: NATIVE_AUDIO_SERVICE_ID,
        provider_route: NATIVE_AUDIO_PROVIDER_ROUTE,
        capability: AUDIO_BACKEND_CAPABILITY_ID,
        priority: NATIVE_AUDIO_PRIORITY,
        owner: NATIVE_AUDIO_OWNER,
        service,
    }) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "audio provider registered gateway='{}' route='{}' priority={} formats='wav,mp3,ogg,flac' spatial=true device_init='async'",
                ENGINE_AUDIO_SERVICE_ID,
                NATIVE_AUDIO_PROVIDER_ROUTE,
                NATIVE_AUDIO_PRIORITY
            );
            true
        }
        Err(error) => {
            // Transactional publication leaves no partial live topology on failure; callers may
            // safely retry in the same HostContext after the owning transaction rolls back.
            newengine_ulog_api::ulog::warn!(
                "audio provider registration failed route='{}' err='{}'",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            false
        }
    }
}
