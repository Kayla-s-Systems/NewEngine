#![forbid(unsafe_op_in_unsafe_fn)]

mod block_render;
mod streaming_asset;
mod streaming_pcm;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::AssetServiceClient;
use newengine_audio_api::{
    sanitize_gain, sanitize_speed, AudioAcousticState, AudioAttenuationSettings,
    AudioConcurrencyScope, AudioCuePlayRequest, AudioCuePreloadRequest, AudioDiagnostics,
    AudioDirectPathResponse, AudioEarlyReflectionField, AudioEarlyReflectionTap,
    AudioEnvironmentState, AudioFeedbackAck, AudioFeedbackEvent, AudioListenerState,
    AudioParameterSet, AudioPlayAck, AudioPlayRequest, AudioPreloadAck, AudioPreloadRequest,
    AudioRenderClock, AudioReverbPreset, AudioReverbSend, AudioRouteGainAck, AudioRouteGainRequest,
    AudioRouteId, AudioServiceInfo, AudioSpatialParams, AudioStopVoiceRequest,
    AudioStreamBufferConfig, AudioStreamPlayRequest, AudioVoiceAck, AudioVoiceBudgetAck,
    AudioVoiceBudgetConfig, AudioVoicePolicy, AudioVoiceRenderAction, AudioVoiceRenderScheduleAck,
    AudioVoiceRenderScheduleRequest, AudioVoiceStealRule, AudioVoiceUpdateRequest, SoundCue,
    SoundCueClip, SoundCueSpatialPolicy, AUDIO_BACKEND_CAPABILITY_ID,
    AUDIO_MAX_EARLY_REFLECTION_TAPS, AUDIO_PROVIDER_ABI_ID, AUDIO_SERVICE_ID,
    AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, AUDIO_SERVICE_METHOD_INVOKE,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1, AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1, AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1, AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1, AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    ENGINE_AUDIO_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use rodio::source::{ChannelVolume, SeekError, SineWave, Source};
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{ChannelCount, Decoder, SampleRate};

use block_render::{native_block_render_graph, BlockVoiceHandle, NativeBlockRenderGraphHandle};
use streaming_asset::RangedAssetReader;
use streaming_pcm::{
    build_streaming_source, probe_stream_source_metadata, StreamSourceMetadata, StreamingStats,
};

pub const NATIVE_AUDIO_SERVICE_ID: &str = AUDIO_SERVICE_ID;
pub const NATIVE_AUDIO_PROVIDER_ROUTE: &str = "engine.audio.native";
pub const NATIVE_AUDIO_OWNER: &str = "newengine-audio-runtime";
pub const NATIVE_AUDIO_PRIORITY: i32 = 100;

const DEFAULT_CLIP_CACHE_LIMIT_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_UI_TONE_GAIN: f32 = 0.10;
const DEFAULT_MAX_PHYSICAL_VOICES: usize = 64;
const MAX_CONFIGURED_PHYSICAL_VOICES: usize = 512;
const MIN_PHYSICAL_AUDIBILITY: f32 = 1.0e-4;
/// Symphonia-backed decoders can reject sub-frame/sub-packet random access near zero.
/// A voice promoted this early is perceptually equivalent to starting at sample zero.
const MIN_MATERIALIZE_SEEK_MS: u64 = 50;
const UI_FEEDBACK_PRIORITY: i32 = 10_000;

// Runtime implementation is split by responsibility while intentionally sharing this crate-level namespace.
include!("audio_runtime/dsp.rs");
include!("audio_runtime/room_buses.rs");
include!("audio_runtime/voices.rs");
include!("audio_runtime/state_core.rs");
include!("audio_runtime/voice_policy.rs");
include!("audio_runtime/sound_graph.rs");
include!("audio_runtime/native_clip.rs");
include!("audio_runtime/state_cache.rs");
include!("audio_runtime/state_playback.rs");
include!("audio_runtime/state_control.rs");
include!("audio_runtime/state_voice_budget.rs");
include!("audio_runtime/state_diagnostics.rs");
include!("audio_runtime/service.rs");
include!("audio_runtime/helpers.rs");

pub const AUDIO_NATIVE_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.audio-native",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Module,
        &["engine.runtime.audio-native"],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_MODULE_TAGS,
    );

struct NativeAudioProviderBootstrapModule;

impl newengine_runtime_unit_api::Module<()> for NativeAudioProviderBootstrapModule {
    fn id(&self) -> &'static str {
        "engine.runtime.audio-native"
    }

    fn init(
        &mut self,
        _ctx: &mut newengine_runtime_unit_api::ModuleCtx<'_, ()>,
    ) -> newengine_runtime_unit_api::EngineResult<()> {
        if !newengine_plugin_host::has_service(NATIVE_AUDIO_SERVICE_ID) {
            let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
            let _ = register_native_audio_provider_best_effort(assets);
        }
        Ok(())
    }
}

fn audio_native_runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    Ok(Some(Box::new(NativeAudioProviderBootstrapModule)))
}

pub const AUDIO_NATIVE_RUNTIME_UNIT_REGISTRATION:
    newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        AUDIO_NATIVE_RUNTIME_UNIT_SPEC,
        audio_native_runtime_unit_factory,
    );

include!("audio_runtime/tests.rs");
