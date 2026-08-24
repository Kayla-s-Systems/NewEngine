#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub use newengine_audio_api::{AcousticSurface, AudioEmitter, AudioEnvironmentZone, AudioPortal};
use newengine_audio_api::{
    AudioAcousticState, AudioCuePlayRequest, AudioEnvironmentState, AudioVoiceUpdateRequest,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ecs::EntityId;
use newengine_transform::{read_entity_world_pose_local_chain, Transform};

use crate::audio_environment::{AudioEnvironmentFrame, AudioEnvironmentResolution};
use crate::audio_gateway::{
    audio_service_info, play_audio_cue, stop_audio_voice, update_audio_voice,
};
use crate::audio_occlusion::AudioOcclusionObservation;
use crate::SceneBridge;

const AUDIO_OCCLUSION_STALE_FIXED_TICKS: u64 = 8;

/// Ephemeral mirror useful to editor diagnostics and runtime inspection.
/// `AudioSceneRuntimeModule` remains authoritative for voice ownership.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioEmitterRuntime {
    pub voice_id: Option<u64>,
    pub cue: String,
    pub provider: String,
    pub obstruction: f32,
    pub occlusion: f32,
    pub transmission_gain: f32,
    pub high_frequency_gain: f32,
    pub low_pass_hz: f32,
    pub acoustic_material: String,
    pub acoustic_fixed_tick: u64,
    pub emitter_environment: String,
    pub listener_environment: String,
    pub portal_gain: f32,
    pub source_reverb_send: f32,
    pub listener_reverb_send: f32,
    pub source_reverb_decay_seconds: f32,
    pub listener_reverb_decay_seconds: f32,
}

impl Default for AudioEmitterRuntime {
    fn default() -> Self {
        Self {
            voice_id: None,
            cue: String::new(),
            provider: String::new(),
            obstruction: 0.0,
            occlusion: 0.0,
            transmission_gain: 1.0,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
            acoustic_material: "surface.clear".to_owned(),
            acoustic_fixed_tick: 0,
            emitter_environment: String::new(),
            listener_environment: String::new(),
            portal_gain: 1.0,
            source_reverb_send: 0.0,
            listener_reverb_send: 0.0,
            source_reverb_decay_seconds: 0.1,
            listener_reverb_decay_seconds: 0.1,
        }
    }
}

#[derive(Clone, Debug)]
struct EmitterSnapshot {
    entity: EntityId,
    stable_key: u64,
    emitter: AudioEmitter,
    position: [f32; 3],
    observation: Option<AudioOcclusionObservation>,
}

#[derive(Clone, Debug)]
struct ManagedVoice {
    voice_id: u64,
    cue: String,
}

/// Presentation-cadence ECS bridge from authored `AudioEmitter` components into
/// the stable `engine.audio` gateway.
///
/// The scene lock is held only while snapshotting/applying ECS state. VFS decode,
/// provider calls, and OS audio interaction happen after releasing the world lock.
pub struct AudioSceneRuntimeModule {
    scene: Arc<SceneBridge>,
    managed: HashMap<u64, ManagedVoice>,
    autoplay_armed: HashMap<u64, String>,
    retry_after_tick: HashMap<u64, u64>,
    last_errors: HashMap<u64, String>,
    acoustic: HashMap<u64, AudioAcousticState>,
    environment: HashMap<u64, AudioEnvironmentState>,
    provider: Option<String>,
    services_generation: u64,
    tick: u64,
}

impl AudioSceneRuntimeModule {
    #[inline]
    pub fn new(scene: Arc<SceneBridge>) -> Self {
        Self {
            scene,
            managed: HashMap::new(),
            autoplay_armed: HashMap::new(),
            retry_after_tick: HashMap::new(),
            last_errors: HashMap::new(),
            acoustic: HashMap::new(),
            environment: HashMap::new(),
            provider: None,
            services_generation: u64::MAX,
            tick: 0,
        }
    }

    fn refresh_provider(&mut self) -> bool {
        let generation = newengine_plugin_host::services_generation();
        if generation != self.services_generation {
            self.services_generation = generation;
            let next_provider = audio_service_info()
                .ok()
                .flatten()
                .filter(|info| info.supports_playback())
                .map(|info| info.provider);
            if self.provider != next_provider {
                // Voice ids are provider-local. A route replacement invalidates all
                // cached ids even if the new provider implements the same contract.
                self.managed.clear();
                self.autoplay_armed.clear();
                self.retry_after_tick.clear();
                self.last_errors.clear();
                self.provider = next_provider;
            }
        }
        self.provider.is_some()
    }

    fn snapshot_emitters(&self) -> (Vec<EmitterSnapshot>, AudioEnvironmentFrame) {
        let scene = self.scene.scene();
        let scene = scene.read();
        let world = scene.world();
        let mut out = Vec::new();
        for entity in world.iter_entities() {
            let Some(emitter) = world.get::<AudioEmitter>(entity).cloned() else {
                continue;
            };
            let position = read_entity_world_pose_local_chain(world, entity)
                .map(|(position, _)| [position.x, position.y, position.z])
                .or_else(|| {
                    world.get::<Transform>(entity).map(|transform| {
                        [
                            transform.position.x,
                            transform.position.y,
                            transform.position.z,
                        ]
                    })
                })
                .unwrap_or([0.0; 3]);
            out.push(EmitterSnapshot {
                entity,
                stable_key: entity.stable_u64(),
                observation: world.get::<AudioOcclusionObservation>(entity).cloned(),
                emitter,
                position,
            });
        }
        out.sort_by_key(|snapshot| snapshot.stable_key);
        let environment = AudioEnvironmentFrame::snapshot(world);
        (out, environment)
    }

    fn stop_managed(&mut self, stable_key: u64) {
        if let Some(managed) = self.managed.remove(&stable_key) {
            let _ = stop_audio_voice(managed.voice_id);
        }
    }

    fn record_error(&mut self, stable_key: u64, cue: &str, error: String) {
        if self
            .last_errors
            .get(&stable_key)
            .is_some_and(|previous| previous == &error)
        {
            return;
        }
        self.last_errors.insert(stable_key, error.clone());
        newengine_ulog_api::ulog::warn!(
            "audio emitter: entity_key={} cue='{}' err='{}'",
            stable_key,
            cue,
            error
        );
    }

    fn clear_error(&mut self, stable_key: u64) {
        self.last_errors.remove(&stable_key);
    }

    fn update_acoustic_state(
        &mut self,
        snapshot: &EmitterSnapshot,
        dt: f32,
        current_fixed_tick: u64,
    ) -> AudioAcousticState {
        let settings = snapshot.emitter.occlusion.sanitized();
        let target = if snapshot.emitter.enabled && snapshot.emitter.spatial && settings.enabled {
            snapshot
                .observation
                .as_ref()
                .filter(|observation| {
                    current_fixed_tick.saturating_sub(observation.fixed_tick)
                        <= AUDIO_OCCLUSION_STALE_FIXED_TICKS
                })
                .map(|observation| {
                    settings.acoustic_state_with_material(
                        observation.obstruction,
                        observation.occlusion,
                        observation.material,
                    )
                })
                .unwrap_or_else(AudioAcousticState::clear)
        } else {
            AudioAcousticState::clear()
        };
        let current = self
            .acoustic
            .get(&snapshot.stable_key)
            .copied()
            .unwrap_or_else(AudioAcousticState::clear);
        let next = current.smoothed_towards(
            target,
            dt,
            settings.attack_seconds,
            settings.release_seconds,
        );
        self.acoustic.insert(snapshot.stable_key, next);
        next
    }

    fn update_environment_state(
        &mut self,
        snapshot: &EmitterSnapshot,
        resolution: &AudioEnvironmentResolution,
        dt: f32,
    ) -> AudioEnvironmentState {
        let target = if snapshot.emitter.enabled && snapshot.emitter.spatial {
            resolution.state
        } else {
            AudioEnvironmentState::clear()
        };
        let current = self
            .environment
            .get(&snapshot.stable_key)
            .copied()
            .unwrap_or_else(AudioEnvironmentState::clear);
        let next = current.smoothed_towards(target, dt, resolution.transition_seconds);
        self.environment.insert(snapshot.stable_key, next);
        next
    }

    fn update_emitters(&mut self, dt: f32, current_fixed_tick: u64) {
        self.tick = self.tick.wrapping_add(1);
        let playback_available = self.refresh_provider();
        let (emitters, environment_frame) = self.snapshot_emitters();
        let live_keys = emitters
            .iter()
            .map(|snapshot| snapshot.stable_key)
            .collect::<HashSet<_>>();

        let stale = self
            .managed
            .keys()
            .copied()
            .filter(|stable_key| !live_keys.contains(stable_key))
            .collect::<Vec<_>>();
        for stable_key in stale {
            self.stop_managed(stable_key);
            self.autoplay_armed.remove(&stable_key);
            self.retry_after_tick.remove(&stable_key);
            self.last_errors.remove(&stable_key);
            self.acoustic.remove(&stable_key);
            self.environment.remove(&stable_key);
        }

        let acoustic_by_key = emitters
            .iter()
            .map(|snapshot| {
                (
                    snapshot.stable_key,
                    self.update_acoustic_state(snapshot, dt, current_fixed_tick),
                )
            })
            .collect::<HashMap<_, _>>();

        let environment_resolution_by_key = emitters
            .iter()
            .map(|snapshot| {
                (
                    snapshot.stable_key,
                    environment_frame.resolve(snapshot.position),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut environment_by_key = HashMap::with_capacity(emitters.len());
        for snapshot in &emitters {
            let resolution = environment_resolution_by_key
                .get(&snapshot.stable_key)
                .cloned()
                .unwrap_or_default();
            let state = self.update_environment_state(snapshot, &resolution, dt);
            environment_by_key.insert(snapshot.stable_key, state);
        }

        if !playback_available {
            self.publish_runtime_mirrors(
                &emitters,
                &environment_resolution_by_key,
                &environment_frame,
            );
            return;
        }

        for snapshot in &emitters {
            let stable_key = snapshot.stable_key;
            let cue = snapshot.emitter.cue.trim();
            if !snapshot.emitter.enabled || cue.is_empty() {
                self.stop_managed(stable_key);
                self.autoplay_armed.remove(&stable_key);
                self.retry_after_tick.remove(&stable_key);
                self.clear_error(stable_key);
                self.acoustic.remove(&stable_key);
                self.environment.remove(&stable_key);
                continue;
            }

            if self
                .managed
                .get(&stable_key)
                .is_some_and(|managed| managed.cue != cue)
            {
                self.stop_managed(stable_key);
                self.autoplay_armed.remove(&stable_key);
            }

            if let Some(managed) = self.managed.get(&stable_key).cloned() {
                let request = AudioVoiceUpdateRequest {
                    voice_id: managed.voice_id,
                    gain: Some(snapshot.emitter.sanitized_gain()),
                    speed: None,
                    seek_seconds: None,
                    paused: Some(false),
                    position: snapshot.emitter.spatial.then_some(snapshot.position),
                    acoustic: acoustic_by_key.get(&stable_key).copied(),
                    environment: environment_by_key.get(&stable_key).copied(),
                };
                match update_audio_voice(&request) {
                    Ok(Some(ack)) if ack.accepted => self.clear_error(stable_key),
                    Ok(Some(ack)) => {
                        // A completed one-shot is no longer a live voice. Keep the
                        // autoplay arm so it does not loop itself every frame.
                        self.managed.remove(&stable_key);
                        if !ack.message.is_empty() && ack.message != "voice not found" {
                            self.record_error(stable_key, cue, ack.message);
                        }
                    }
                    Ok(None) => {
                        self.managed.remove(&stable_key);
                    }
                    Err(error) => {
                        self.record_error(stable_key, cue, error);
                    }
                }
                continue;
            }

            if !snapshot.emitter.autoplay
                || self
                    .autoplay_armed
                    .get(&stable_key)
                    .is_some_and(|armed_cue| armed_cue == cue)
                || self
                    .retry_after_tick
                    .get(&stable_key)
                    .is_some_and(|retry| self.tick < *retry)
            {
                continue;
            }

            let mut request = AudioCuePlayRequest::new(cue.to_owned());
            request.gain = snapshot.emitter.sanitized_gain();
            request.position = snapshot.emitter.spatial.then_some(snapshot.position);
            request.seed = Some(stable_key ^ self.tick.rotate_left(17));
            request.acoustic = acoustic_by_key
                .get(&stable_key)
                .copied()
                .unwrap_or_else(AudioAcousticState::clear);
            request.environment = environment_by_key
                .get(&stable_key)
                .copied()
                .unwrap_or_else(AudioEnvironmentState::clear);
            match play_audio_cue(&request) {
                Ok(Some(ack)) if ack.accepted => {
                    if let Some(voice_id) = ack.voice_id {
                        self.managed.insert(
                            stable_key,
                            ManagedVoice {
                                voice_id,
                                cue: cue.to_owned(),
                            },
                        );
                        self.autoplay_armed.insert(stable_key, cue.to_owned());
                        self.retry_after_tick.remove(&stable_key);
                        self.clear_error(stable_key);
                    }
                }
                Ok(Some(ack)) => {
                    self.retry_after_tick
                        .insert(stable_key, self.tick.saturating_add(30));
                    if !ack.message.is_empty() {
                        self.record_error(stable_key, cue, ack.message);
                    }
                }
                Ok(None) => {
                    self.retry_after_tick
                        .insert(stable_key, self.tick.saturating_add(30));
                }
                Err(error) => {
                    self.retry_after_tick
                        .insert(stable_key, self.tick.saturating_add(30));
                    self.record_error(stable_key, cue, error);
                }
            }
        }

        self.publish_runtime_mirrors(
            &emitters,
            &environment_resolution_by_key,
            &environment_frame,
        );
    }

    fn publish_runtime_mirrors(
        &self,
        emitters: &[EmitterSnapshot],
        environment_resolution_by_key: &HashMap<u64, AudioEnvironmentResolution>,
        environment_frame: &AudioEnvironmentFrame,
    ) {
        let provider = self.provider.clone().unwrap_or_default();
        let scene = self.scene.scene();
        let mut scene = scene.write();
        let world = scene.world_mut();
        world.insert_resource(environment_frame.runtime_state());
        for snapshot in emitters {
            if !world.exists(snapshot.entity) {
                continue;
            }
            let managed = self.managed.get(&snapshot.stable_key);
            let acoustic = self
                .acoustic
                .get(&snapshot.stable_key)
                .copied()
                .unwrap_or_else(AudioAcousticState::clear);
            let environment = self
                .environment
                .get(&snapshot.stable_key)
                .copied()
                .unwrap_or_else(AudioEnvironmentState::clear);
            let environment_resolution = environment_resolution_by_key
                .get(&snapshot.stable_key)
                .cloned()
                .unwrap_or_default();
            let runtime = AudioEmitterRuntime {
                voice_id: managed.map(|managed| managed.voice_id),
                cue: snapshot.emitter.cue.clone(),
                provider: provider.clone(),
                obstruction: acoustic.obstruction,
                occlusion: acoustic.occlusion,
                transmission_gain: acoustic.transmission_gain,
                high_frequency_gain: acoustic.high_frequency_gain,
                low_pass_hz: acoustic.low_pass_hz,
                acoustic_material: snapshot
                    .observation
                    .as_ref()
                    .map(|observation| observation.dominant_material.clone())
                    .unwrap_or_else(|| "surface.clear".to_owned()),
                acoustic_fixed_tick: snapshot
                    .observation
                    .as_ref()
                    .map(|observation| observation.fixed_tick)
                    .unwrap_or(0),
                emitter_environment: environment_resolution.emitter_zone,
                listener_environment: environment_resolution.listener_zone,
                portal_gain: environment.portal_gain,
                source_reverb_send: environment.source_send.gain,
                listener_reverb_send: environment.listener_send.gain,
                source_reverb_decay_seconds: environment.source_send.preset.decay_seconds,
                listener_reverb_decay_seconds: environment.listener_send.preset.decay_seconds,
            };
            let _ = world.insert(snapshot.entity, runtime);
        }
    }

    fn stop_all(&mut self) {
        let voices = self
            .managed
            .drain()
            .map(|(_, managed)| managed.voice_id)
            .collect::<Vec<_>>();
        for voice_id in voices {
            let _ = stop_audio_voice(voice_id);
        }
        self.autoplay_armed.clear();
        self.retry_after_tick.clear();
        self.last_errors.clear();
        self.acoustic.clear();
        self.environment.clear();
    }
}

impl<E: Send + 'static> Module<E> for AudioSceneRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.audio.scene-runtime"
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let (dt, fixed_tick) = ctx
            .frame()
            .map(|frame| (frame.dt, frame.fixed_tick))
            .unwrap_or((1.0 / 60.0, 0));
        self.update_emitters(dt, fixed_tick);
        Ok(())
    }

    fn shutdown(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.stop_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_defaults_to_enabled_spatial_autoplay() {
        let emitter = AudioEmitter::new("shared/audio/ambience/ambience.yscd@wind");
        assert!(emitter.enabled);
        assert!(emitter.autoplay);
        assert!(emitter.spatial);
        assert_eq!(emitter.sanitized_gain(), 1.0);
    }

    #[test]
    fn emitter_gain_is_sanitized_before_crossing_provider_boundary() {
        let mut emitter = AudioEmitter::default();
        emitter.gain = f32::INFINITY;
        assert_eq!(emitter.sanitized_gain(), 1.0);
        emitter.gain = 99.0;
        assert_eq!(emitter.sanitized_gain(), 4.0);
    }

    #[test]
    fn emitter_runtime_defaults_to_clear_acoustic_transmission() {
        let runtime = AudioEmitterRuntime::default();
        assert_eq!(runtime.obstruction, 0.0);
        assert_eq!(runtime.occlusion, 0.0);
        assert_eq!(runtime.transmission_gain, 1.0);
    }
}
