#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use abi_stable::std_types::{RResult, RString};
use newengine_audio_api::{
    sanitize_gain, sanitize_speed, AudioBus, AudioBusGainAck, AudioBusGainRequest,
    AudioDiagnostics, AudioFeedbackAck, AudioFeedbackEvent, AudioListenerState, AudioPlayAck,
    AudioPlayRequest, AudioPreloadAck, AudioPreloadRequest, AudioServiceInfo,
    AudioStopVoiceRequest, AudioVoiceAck, AudioVoiceUpdateRequest, AUDIO_BACKEND_CAPABILITY_ID,
    AUDIO_PROVIDER_ABI_ID, AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, AUDIO_SERVICE_METHOD_INVOKE,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1, AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1, AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1, AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    ENGINE_AUDIO_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use rodio::source::{SineWave, Source};
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{Decoder, Player, SpatialPlayer};

pub const NATIVE_AUDIO_SERVICE_ID: &str = "newengine.audio.native";
pub const NATIVE_AUDIO_PROVIDER_ROUTE: &str = "engine.audio.native";
pub const NATIVE_AUDIO_OWNER: &str = "newengine-audio-runtime";
pub const NATIVE_AUDIO_PRIORITY: i32 = 100;

const DEFAULT_CLIP_CACHE_LIMIT_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_UI_TONE_GAIN: f32 = 0.10;

static AUDIO_RUNTIME_REGISTERED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct CachedClip {
    bytes: Arc<[u8]>,
}

impl CachedClip {
    #[inline]
    fn len(&self) -> usize {
        self.bytes.len()
    }
}

enum VoiceControl {
    Flat(Player),
    Spatial(SpatialPlayer),
}

impl VoiceControl {
    #[inline]
    fn set_volume(&self, value: f32) {
        match self {
            Self::Flat(player) => player.set_volume(value),
            Self::Spatial(player) => player.set_volume(value),
        }
    }

    #[inline]
    fn set_speed(&self, value: f32) {
        match self {
            Self::Flat(player) => player.set_speed(value),
            Self::Spatial(player) => player.set_speed(value),
        }
    }

    #[inline]
    fn set_paused(&self, paused: bool) {
        match (self, paused) {
            (Self::Flat(player), true) => player.pause(),
            (Self::Flat(player), false) => player.play(),
            (Self::Spatial(player), true) => player.pause(),
            (Self::Spatial(player), false) => player.play(),
        }
    }

    #[inline]
    fn set_emitter_position(&self, position: [f32; 3]) -> bool {
        match self {
            Self::Spatial(player) => {
                player.set_emitter_position(position);
                true
            }
            Self::Flat(_) => false,
        }
    }

    #[inline]
    fn update_listener(&self, listener: AudioListenerState) {
        if let Self::Spatial(player) = self {
            let (left, right) = listener.ear_positions();
            player.set_left_ear_position(left);
            player.set_right_ear_position(right);
        }
    }

    #[inline]
    fn stop(&self) {
        match self {
            Self::Flat(player) => player.stop(),
            Self::Spatial(player) => player.stop(),
        }
    }

    #[inline]
    fn empty(&self) -> bool {
        match self {
            Self::Flat(player) => player.empty(),
            Self::Spatial(player) => player.empty(),
        }
    }

    #[inline]
    fn is_spatial(&self) -> bool {
        matches!(self, Self::Spatial(_))
    }
}

struct VoiceEntry {
    control: VoiceControl,
    bus: AudioBus,
    gain: f32,
}

pub struct AudioRuntimeState {
    output: MixerDeviceSink,
    voices: HashMap<u64, VoiceEntry>,
    next_voice_id: u64,
    listener: AudioListenerState,
    bus_gains: BTreeMap<AudioBus, f32>,
    clips: HashMap<String, CachedClip>,
    cached_bytes: usize,
    cache_limit_bytes: usize,
}

impl AudioRuntimeState {
    pub fn open_default() -> Result<Self, String> {
        let mut output = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("open default audio output failed: {error}"))?;
        output.log_on_drop(false);
        let mut bus_gains = BTreeMap::new();
        for bus in AudioBus::all() {
            bus_gains.insert(bus, 1.0);
        }
        Ok(Self {
            output,
            voices: HashMap::new(),
            next_voice_id: 1,
            listener: AudioListenerState::default(),
            bus_gains,
            clips: HashMap::new(),
            cached_bytes: 0,
            cache_limit_bytes: cache_limit_bytes_from_env(),
        })
    }

    #[inline]
    fn alloc_voice_id(&mut self) -> u64 {
        let id = self.next_voice_id.max(1);
        self.next_voice_id = id.wrapping_add(1).max(1);
        id
    }

    fn prune_finished(&mut self) {
        self.voices.retain(|_, voice| !voice.control.empty());
    }

    #[inline]
    fn bus_gain(&self, bus: AudioBus) -> f32 {
        let master = self
            .bus_gains
            .get(&AudioBus::Master)
            .copied()
            .unwrap_or(1.0);
        if bus == AudioBus::Master {
            master
        } else {
            master * self.bus_gains.get(&bus).copied().unwrap_or(1.0)
        }
    }

    #[inline]
    fn effective_voice_gain(&self, bus: AudioBus, voice_gain: f32) -> f32 {
        sanitize_gain(voice_gain) * self.bus_gain(bus)
    }

    fn refresh_voice_gains(&self) {
        for voice in self.voices.values() {
            voice
                .control
                .set_volume(self.effective_voice_gain(voice.bus, voice.gain));
        }
    }

    fn preload(&mut self, request: AudioPreloadRequest) -> Result<AudioPreloadAck, String> {
        let uri = normalize_uri(&request.clip.uri)?;
        if let Some(existing) = self.clips.get(&uri) {
            return Ok(AudioPreloadAck {
                accepted: true,
                cached: true,
                bytes: existing.len(),
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            });
        }

        let path = resolve_file_path(&uri)?;
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("audio clip read failed '{}': {error}", path.display()))?;
        if bytes.is_empty() {
            return Err(format!("audio clip is empty: '{}'", path.display()));
        }
        if bytes.len() > self.cache_limit_bytes {
            return Err(format!(
                "audio clip '{}' is {} bytes and exceeds cache limit {} bytes",
                path.display(),
                bytes.len(),
                self.cache_limit_bytes
            ));
        }

        self.make_cache_room(bytes.len());
        let bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let len = bytes.len();
        self.clips.insert(uri, CachedClip { bytes });
        self.cached_bytes = self.cached_bytes.saturating_add(len);
        Ok(AudioPreloadAck {
            accepted: true,
            cached: false,
            bytes: len,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
        })
    }

    fn make_cache_room(&mut self, incoming: usize) {
        if self.cached_bytes.saturating_add(incoming) <= self.cache_limit_bytes {
            return;
        }
        // V1 uses a deterministic all-or-nothing eviction. LRU/residency belongs
        // in the shared asset/VFS layer rather than leaking into the provider API.
        self.clips.clear();
        self.cached_bytes = 0;
    }

    fn clip_bytes(&mut self, uri: &str) -> Result<Arc<[u8]>, String> {
        let normalized = normalize_uri(uri)?;
        if !self.clips.contains_key(&normalized) {
            self.preload(AudioPreloadRequest {
                clip: newengine_audio_api::AudioClipRef::new(normalized.clone()),
            })?;
        }
        self.clips
            .get(&normalized)
            .map(|clip| Arc::clone(&clip.bytes))
            .ok_or_else(|| format!("audio clip cache admission failed: '{normalized}'"))
    }

    fn play_clip(&mut self, request: AudioPlayRequest) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        let request = request.sanitized();
        let clip_bytes = self.clip_bytes(&request.clip.uri)?;
        let decoder = Decoder::try_from(Cursor::new(clip_bytes))
            .map_err(|error| format!("audio decode failed '{}': {error}", request.clip.uri))?;
        let volume = self.effective_voice_gain(request.bus, request.gain);
        let speed = sanitize_speed(request.speed);
        let voice_id = self.alloc_voice_id();

        let control = if let Some(spatial) = request.spatial {
            let (left, right) = self.listener.ear_positions();
            let player = SpatialPlayer::connect_new(
                self.output.mixer(),
                spatial.sanitized().position,
                left,
                right,
            );
            player.set_volume(volume);
            player.set_speed(speed);
            if request.looping {
                player.append(decoder.repeat_infinite());
            } else {
                player.append(decoder);
            }
            VoiceControl::Spatial(player)
        } else {
            let player = Player::connect_new(self.output.mixer());
            player.set_volume(volume);
            player.set_speed(speed);
            if request.looping {
                player.append(decoder.repeat_infinite());
            } else {
                player.append(decoder);
            }
            VoiceControl::Flat(player)
        };

        self.voices.insert(
            voice_id,
            VoiceEntry {
                control,
                bus: request.bus,
                gain: request.gain,
            },
        );
        Ok(AudioPlayAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: Some(voice_id),
            message: String::new(),
        })
    }

    fn play_feedback(&mut self, event: AudioFeedbackEvent) -> AudioFeedbackAck {
        self.prune_finished();
        let (frequency, duration_ms) = feedback_tone(&event.id);
        let gain = sanitize_gain(DEFAULT_UI_TONE_GAIN * event.intensity.clamp(0.0, 1.0));
        let player = Player::connect_new(self.output.mixer());
        player.set_volume(self.effective_voice_gain(AudioBus::Ui, gain));
        player.append(
            SineWave::new(frequency)
                .take_duration(Duration::from_millis(duration_ms))
                .fade_out(Duration::from_millis((duration_ms / 2).max(8))),
        );
        let voice_id = self.alloc_voice_id();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: VoiceControl::Flat(player),
                bus: AudioBus::Ui,
                gain,
            },
        );
        AudioFeedbackAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            queued_events: self.voices.len(),
        }
    }

    fn stop_voice(&mut self, request: AudioStopVoiceRequest) -> AudioVoiceAck {
        self.prune_finished();
        match self.voices.remove(&request.voice_id) {
            Some(voice) => {
                voice.control.stop();
                AudioVoiceAck {
                    accepted: true,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: String::new(),
                }
            }
            None => AudioVoiceAck {
                accepted: false,
                voice_id: request.voice_id,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                message: "voice not found".to_owned(),
            },
        }
    }

    fn update_voice(&mut self, request: AudioVoiceUpdateRequest) -> AudioVoiceAck {
        self.prune_finished();
        let master_gain = self
            .bus_gains
            .get(&AudioBus::Master)
            .copied()
            .unwrap_or(1.0);
        let Some(voice) = self.voices.get_mut(&request.voice_id) else {
            return AudioVoiceAck {
                accepted: false,
                voice_id: request.voice_id,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                message: "voice not found".to_owned(),
            };
        };

        if let Some(gain) = request.gain {
            voice.gain = sanitize_gain(gain);
            let bus_gain = if voice.bus == AudioBus::Master {
                master_gain
            } else {
                master_gain * self.bus_gains.get(&voice.bus).copied().unwrap_or(1.0)
            };
            voice.control.set_volume(voice.gain * bus_gain);
        }
        if let Some(speed) = request.speed {
            voice.control.set_speed(sanitize_speed(speed));
        }
        if let Some(paused) = request.paused {
            voice.control.set_paused(paused);
        }
        if let Some(position) = request.position {
            if !voice
                .control
                .set_emitter_position(sanitize_position(position))
            {
                return AudioVoiceAck {
                    accepted: false,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: "position update requires a spatial voice".to_owned(),
                };
            }
        }
        AudioVoiceAck {
            accepted: true,
            voice_id: request.voice_id,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            message: String::new(),
        }
    }

    fn set_listener(&mut self, listener: AudioListenerState) -> AudioListenerState {
        self.listener = listener.sanitized();
        for voice in self.voices.values() {
            voice.control.update_listener(self.listener);
        }
        self.listener
    }

    fn set_bus_gain(&mut self, request: AudioBusGainRequest) -> AudioBusGainAck {
        let gain = sanitize_gain(request.gain);
        self.bus_gains.insert(request.bus, gain);
        self.refresh_voice_gains();
        AudioBusGainAck {
            accepted: true,
            bus: request.bus,
            gain,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
        }
    }

    fn diagnostics(&mut self) -> AudioDiagnostics {
        self.prune_finished();
        AudioDiagnostics {
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            output_ready: true,
            active_voices: self.voices.len(),
            spatial_voices: self
                .voices
                .values()
                .filter(|voice| voice.control.is_spatial())
                .count(),
            cached_clips: self.clips.len(),
            cached_bytes: self.cached_bytes,
            listener: self.listener,
            bus_gains: self
                .bus_gains
                .iter()
                .map(|(bus, gain)| (bus.as_str().to_owned(), *gain))
                .collect(),
        }
    }

    fn shutdown(&mut self) {
        for voice in self.voices.values() {
            voice.control.stop();
        }
        self.voices.clear();
        self.clips.clear();
        self.cached_bytes = 0;
    }
}

fn audio_service(state: AudioRuntimeState) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = AudioServiceInfo::playback_provider(NATIVE_AUDIO_PROVIDER_ROUTE);
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
        "audio-buses",
        "clip-cache",
    ])
    .notes("First-party native audio provider; replaceable through engine.audio gateway routing.");

    JsonServiceRouter::with_state(NATIVE_AUDIO_SERVICE_ID, state)
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
            AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
            |state, request: AudioPlayRequest| state.play_clip(request),
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
            AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
            |state, request: AudioBusGainRequest| state.set_bus_gain(request),
        )
        .get_json(AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, |state| {
            state.diagnostics()
        })
        .blob(AUDIO_SERVICE_METHOD_SHUTDOWN_V1, |state, _payload: Blob| {
            state.shutdown();
            ok_empty_blob()
        })
        .into_service_v1()
}

/// Registers the first-party native provider when an OS audio output is usable.
/// Failure is non-fatal: the semantic queue route remains active for headless,
/// servers, CI, and machines without a sound device.
pub fn register_native_audio_provider_best_effort() -> bool {
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

    if AUDIO_RUNTIME_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return true;
    }

    let state = match AudioRuntimeState::open_default() {
        Ok(state) => state,
        Err(error) => {
            AUDIO_RUNTIME_REGISTERED.store(false, Ordering::Release);
            newengine_ulog_api::ulog::warn!(
                "audio provider unavailable route='{}' err='{}'; engine.audio fallback remains active",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            return false;
        }
    };

    let service = audio_service(state);
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
                "audio provider registered gateway='{}' route='{}' priority={} formats='wav,mp3,ogg,flac' spatial=true",
                ENGINE_AUDIO_SERVICE_ID,
                NATIVE_AUDIO_PROVIDER_ROUTE,
                NATIVE_AUDIO_PRIORITY
            );
            true
        }
        Err(error) => {
            // The service may already have been registered before a route-level
            // validation failure. Keep the one-shot guard set to avoid duplicate
            // service registration on a later best-effort call.
            newengine_ulog_api::ulog::warn!(
                "audio provider registration failed route='{}' err='{}'",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            false
        }
    }
}

fn normalize_uri(uri: &str) -> Result<String, String> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err("audio clip uri is empty".to_owned());
    }
    Ok(trimmed
        .strip_prefix("file://")
        .unwrap_or(trimmed)
        .to_owned())
}

fn resolve_file_path(uri: &str) -> Result<PathBuf, String> {
    let path = Path::new(uri);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("resolve current directory for audio clip failed: {error}"))?
            .join(path)
    };
    if !resolved.is_file() {
        return Err(format!("audio clip not found: '{}'", resolved.display()));
    }
    Ok(resolved)
}

#[inline]
fn sanitize_position(position: [f32; 3]) -> [f32; 3] {
    position.map(|component| {
        if component.is_finite() {
            component
        } else {
            0.0
        }
    })
}

fn feedback_tone(event_id: &str) -> (f32, u64) {
    match event_id {
        "ui.open" => (660.0, 55),
        "ui.close" => (440.0, 50),
        "ui.navigate" => (520.0, 30),
        "ui.confirm" => (780.0, 70),
        "ui.back" => (390.0, 55),
        "ui.rebind" => (880.0, 85),
        "ui.error" => (220.0, 120),
        _ => (500.0, 35),
    }
}

fn cache_limit_bytes_from_env() -> usize {
    std::env::var("NEWENGINE_AUDIO_CACHE_MB")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|mb| mb.clamp(8, 2048).saturating_mul(1024 * 1024))
        .unwrap_or(DEFAULT_CLIP_CACHE_LIMIT_BYTES)
}

#[inline]
fn headless_runtime() -> bool {
    env_flag("NEWENGINE_HEADLESS")
}

#[inline]
fn audio_disabled_by_env() -> bool {
    env_flag("NEWENGINE_AUDIO_DISABLED")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_tones_are_bounded() {
        for event in [
            "ui.open",
            "ui.close",
            "ui.navigate",
            "ui.confirm",
            "ui.back",
            "ui.rebind",
            "ui.error",
        ] {
            let (hz, ms) = feedback_tone(event);
            assert!((80.0..=4_000.0).contains(&hz));
            assert!((10..=500).contains(&ms));
        }
    }

    #[test]
    fn file_uri_is_normalized_without_changing_plain_paths() {
        assert_eq!(
            normalize_uri("file://C:/audio/test.wav").unwrap(),
            "C:/audio/test.wav"
        );
        assert_eq!(normalize_uri("audio/test.wav").unwrap(), "audio/test.wav");
        assert!(normalize_uri("  ").is_err());
    }
}
