#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{HashMap, HashSet};

pub use newengine_audio_api::{AcousticSurface, AudioEmitter, AudioEnvironmentZone, AudioPortal};
use newengine_audio_api::{
    AudioAcousticState, AudioCuePlayRequest, AudioEnvironmentState, AudioOcclusionSettings,
    AudioVoiceUpdateRequest,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ecs::EntityId;
use newengine_transform::{read_entity_world_pose_local_chain, Transform};

use crate::audio_environment::{AudioEnvironmentFrame, AudioEnvironmentResolution};
use crate::AudioWorldScene;
use newengine_audio_client::{
    audio_service_info, play_audio_cue, stop_audio_voice, update_audio_voice,
};
use newengine_audio_world_api::{
    AudioEdgeDiffractionObservation, AudioEdgeDiffractionPathObservation, AudioEmitterRuntime,
    AudioListenerRuntimeState, AudioOcclusionObservation,
};

const AUDIO_OCCLUSION_STALE_FIXED_TICKS: u64 = 8;
const AUDIO_DIFFRACTION_STALE_FIXED_TICKS: u64 = 8;
const AUDIO_DIFFRACTION_POSITION_EPSILON_M: f32 = 0.75;
const SPEED_OF_SOUND_MPS: f32 = 343.0;

include!("audio_scene/acoustic_route.rs");

#[derive(Clone, Debug)]
struct EmitterSnapshot {
    entity: EntityId,
    stable_key: u64,
    emitter: AudioEmitter,
    position: [f32; 3],
    listener_position: [f32; 3],
    observation: Option<AudioOcclusionObservation>,
    diffraction_observation: Option<AudioEdgeDiffractionObservation>,
}

#[derive(Clone, Debug)]
struct EmitterFrameState {
    snapshot: EmitterSnapshot,
    acoustic: AudioAcousticState,
    environment_resolution: AudioEnvironmentResolution,
    environment: AudioEnvironmentState,
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
    scene: AudioWorldScene,
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
    pub fn new(scene: AudioWorldScene) -> Self {
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
        let listener_position = world
            .resource::<AudioListenerRuntimeState>()
            .map(|state| state.listener.sanitized().position)
            .unwrap_or([0.0; 3]);
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
                diffraction_observation: world
                    .get::<AudioEdgeDiffractionObservation>(entity)
                    .cloned(),
                emitter,
                position,
                listener_position,
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
    ) -> EffectiveAcousticRoute {
        let settings = snapshot.emitter.occlusion.sanitized();
        let target_route = if snapshot.emitter.enabled && snapshot.emitter.spatial {
            resolve_effective_acoustic_route(
                settings,
                snapshot.observation.as_ref(),
                snapshot.diffraction_observation.as_ref(),
                snapshot.position,
                snapshot.listener_position,
                current_fixed_tick,
            )
        } else {
            EffectiveAcousticRoute::clear()
        };
        let current = self
            .acoustic
            .get(&snapshot.stable_key)
            .copied()
            .unwrap_or_else(AudioAcousticState::clear);
        let next = current.smoothed_towards(
            target_route.acoustic,
            dt,
            settings.attack_seconds,
            settings.release_seconds,
        );
        self.acoustic.insert(snapshot.stable_key, next);
        EffectiveAcousticRoute {
            acoustic: next,
            ..target_route
        }
    }

    fn update_environment_state(
        &mut self,
        snapshot: &EmitterSnapshot,
        resolution: &AudioEnvironmentResolution,
        acoustic: AudioAcousticState,
        detour_delay_ms: f32,
        dt: f32,
    ) -> AudioEnvironmentState {
        let target = if snapshot.emitter.enabled && snapshot.emitter.spatial {
            environment_with_effective_direct_route(resolution.state, acoustic, detour_delay_ms)
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
        if !emitters.is_empty() && (self.tick <= 3 || self.tick % 300 == 0) {
            newengine_ulog_api::ulog::info!(
                "audio scene runtime: tick={} emitters={} playback_available={} provider='{}' managed={} autoplay_armed={} policy='throttled-runtime-health'",
                self.tick,
                emitters.len(),
                playback_available,
                self.provider.as_deref().unwrap_or(""),
                self.managed.len(),
                self.autoplay_armed.len(),
            );
        }
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

        // Keep all frame-local derived state adjacent to the emitter snapshot. The old
        // path materialized four HashMaps keyed by the same stable id and then paid
        // repeated hash lookups during update/playback/mirror publication.
        let mut frame_emitters = Vec::with_capacity(emitters.len());
        for snapshot in emitters {
            let route = self.update_acoustic_state(&snapshot, dt, current_fixed_tick);
            let environment_resolution =
                environment_frame.resolve_for_emitter(snapshot.stable_key, snapshot.position);
            let environment = self.update_environment_state(
                &snapshot,
                &environment_resolution,
                route.acoustic,
                route.detour_delay_ms,
                dt,
            );
            frame_emitters.push(EmitterFrameState {
                snapshot,
                acoustic: route.acoustic,
                environment_resolution,
                environment,
            });
        }

        if !playback_available {
            self.publish_runtime_mirrors(&frame_emitters, &environment_frame);
            return;
        }

        for frame in &mut frame_emitters {
            let snapshot = &frame.snapshot;
            let stable_key = snapshot.stable_key;
            let cue = snapshot.emitter.cue.trim();
            if !snapshot.emitter.enabled || cue.is_empty() {
                self.stop_managed(stable_key);
                self.autoplay_armed.remove(&stable_key);
                self.retry_after_tick.remove(&stable_key);
                self.clear_error(stable_key);
                self.acoustic.remove(&stable_key);
                self.environment.remove(&stable_key);
                // Match the historical mirror path: once an enabled playback route is
                // explicitly torn down, its published acoustic/environment state is clear.
                frame.acoustic = AudioAcousticState::clear();
                frame.environment = AudioEnvironmentState::clear();
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
                    acoustic: Some(frame.acoustic),
                    environment: Some(frame.environment),
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
            request.scope_id = Some(stable_key);
            request.acoustic = frame.acoustic;
            request.environment = frame.environment;
            match play_audio_cue(&request) {
                Ok(Some(ack)) if ack.accepted => {
                    newengine_ulog_api::ulog::info!(
                        "audio emitter autoplay accepted entity_key={} cue={} provider={} voice_id={:?} voice_ids={:?} virtualized={} message={} diagnostics={:?}",
                        stable_key,
                        cue,
                        ack.provider,
                        ack.voice_id,
                        ack.voice_ids,
                        ack.virtualized,
                        ack.message,
                        ack.diagnostics,
                    );
                    let waiting_for_output = ack.virtualized
                        && ack.diagnostics.iter().any(|line| {
                            line.contains("output_state='initializing'")
                                || line.contains("audio output device is still initializing")
                        });
                    if waiting_for_output {
                        if let Some(voice_id) = ack.voice_id {
                            let _ = stop_audio_voice(voice_id);
                        }
                        self.retry_after_tick
                            .insert(stable_key, self.tick.saturating_add(1));
                        self.clear_error(stable_key);
                        continue;
                    }
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

        self.publish_runtime_mirrors(&frame_emitters, &environment_frame);
    }

    fn publish_runtime_mirrors(
        &self,
        emitters: &[EmitterFrameState],
        environment_frame: &AudioEnvironmentFrame,
    ) {
        let provider = self.provider.clone().unwrap_or_default();
        let scene = self.scene.scene();
        let mut scene = scene.write();
        let world = scene.world_mut();
        world.insert_resource(environment_frame.runtime_state());
        for frame in emitters {
            let snapshot = &frame.snapshot;
            if !world.exists(snapshot.entity) {
                continue;
            }
            let managed = self.managed.get(&snapshot.stable_key);
            let acoustic = frame.acoustic;
            let environment = frame.environment;
            let environment_resolution = &frame.environment_resolution;
            let runtime = AudioEmitterRuntime {
                voice_id: managed.map(|managed| managed.voice_id),
                cue: snapshot.emitter.cue.clone(),
                provider: provider.clone(),
                obstruction: acoustic.obstruction,
                occlusion: acoustic.occlusion,
                estimated_occluder_thickness_m: snapshot
                    .observation
                    .as_ref()
                    .map(|observation| observation.estimated_thickness_m)
                    .unwrap_or(0.0),
                center_blocker_layers: snapshot
                    .observation
                    .as_ref()
                    .map(|observation| observation.center_blocker_layers)
                    .unwrap_or(0),
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
                emitter_environment: environment_resolution.emitter_zone.clone(),
                listener_environment: environment_resolution.listener_zone.clone(),
                portal_gain: environment.portal_gain,
                direct_path_gain: environment.direct_path.gain,
                direct_path_high_frequency_gain: environment.direct_path.high_frequency_gain,
                direct_path_low_pass_hz: environment.direct_path.low_pass_hz,
                direct_path_extra_delay_ms: environment.direct_path.extra_delay_ms,
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
include!("audio_scene/tests.rs");
