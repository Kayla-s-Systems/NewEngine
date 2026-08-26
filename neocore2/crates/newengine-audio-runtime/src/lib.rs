#![forbid(unsafe_op_in_unsafe_fn)]

mod streaming_asset;
mod streaming_pcm;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::AssetServiceClient;
use newengine_audio_api::{
    sanitize_gain, sanitize_speed, AudioAcousticState, AudioAttenuationSettings, AudioBus,
    AudioBusGainAck, AudioBusGainRequest, AudioCuePlayRequest, AudioCuePreloadRequest,
    AudioDiagnostics, AudioEnvironmentState, AudioFeedbackAck, AudioFeedbackEvent,
    AudioListenerState, AudioPlayAck, AudioPlayRequest, AudioPreloadAck, AudioPreloadRequest,
    AudioReverbSend, AudioServiceInfo, AudioSpatialParams, AudioStopVoiceRequest,
    AudioStreamBufferConfig, AudioStreamPlayRequest, AudioVoiceAck, AudioVoiceUpdateRequest,
    SoundCue, SoundCueClip, SoundCueSpatialPolicy, AUDIO_BACKEND_CAPABILITY_ID,
    AUDIO_PROVIDER_ABI_ID, AUDIO_SERVICE_ID, AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1,
    AUDIO_SERVICE_METHOD_INVOKE, AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1, AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1, AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1, AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1, AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    ENGINE_AUDIO_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use rodio::source::{SeekError, SineWave, Source};
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{ChannelCount, Decoder, Player, SampleRate, SpatialPlayer};

use streaming_asset::RangedAssetReader;
use streaming_pcm::{build_streaming_source, StreamingStats};

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
include!("audio_runtime/voices.rs");
include!("audio_runtime/state_core.rs");
include!("audio_runtime/state_cache.rs");
include!("audio_runtime/state_playback.rs");
include!("audio_runtime/state_control.rs");
include!("audio_runtime/state_voice_budget.rs");
include!("audio_runtime/state_diagnostics.rs");
include!("audio_runtime/service.rs");
include!("audio_runtime/helpers.rs");
include!("audio_runtime/tests.rs");
