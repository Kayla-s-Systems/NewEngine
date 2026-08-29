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

#[derive(Clone, Copy, Debug, PartialEq)]
struct EffectiveAcousticRoute {
    acoustic: AudioAcousticState,
    detour_delay_ms: f32,
    used_diffraction: bool,
}

impl EffectiveAcousticRoute {
    #[inline]
    const fn clear() -> Self {
        Self {
            acoustic: AudioAcousticState::clear(),
            detour_delay_ms: 0.0,
            used_diffraction: false,
        }
    }
}

#[inline]
fn array_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).max(0.0).sqrt()
}

fn diffraction_path_acoustic_state(
    path: &AudioEdgeDiffractionPathObservation,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    obstruction: f32,
    occlusion: f32,
) -> Option<AudioAcousticState> {
    if !path.visible
        || !path.path_length_m.is_finite()
        || !path.excess_length_m.is_finite()
        || !path.bend_angle_radians.is_finite()
        || !path.wedge_angle_radians.is_finite()
    {
        return None;
    }
    let direct = array_distance(source_position, listener_position);
    if !direct.is_finite() || direct <= 1.0e-4 || path.path_length_m + 1.0e-4 < direct {
        return None;
    }

    let excess = path.excess_length_m.max(0.0);
    let bend = path.bend_angle_radians.clamp(0.0, std::f32::consts::PI) / std::f32::consts::PI;
    let wedge_sharpness = (1.0
        - path.wedge_angle_radians.clamp(0.0, std::f32::consts::PI) / std::f32::consts::PI)
        .clamp(0.0, 1.0);
    let spreading = (direct / path.path_length_m.max(direct))
        .clamp(0.05, 1.0)
        .sqrt();
    let excess_gain = (-0.22 * excess).exp();
    let bend_gain = (-0.85 * bend * bend).exp();
    let wedge_gain = (1.0 - 0.12 * wedge_sharpness).clamp(0.70, 1.0);

    // Edge diffraction is an alternate route around the blocker. Deliberately use boundary
    // hardness/absorption here, never material.transmission_gain: that field describes energy
    // travelling through the blocker and would double-apply wall transmission to the bypass.
    let material = path.material.sanitized();
    let boundary_gain = if path.material_known {
        0.72 + 0.28 * material.reflection_gain
    } else {
        1.0
    };
    let broadband =
        (spreading * excess_gain * bend_gain * wedge_gain * boundary_gain).clamp(0.0, 1.0);

    let material_hf = if path.material_known {
        0.35 + 0.65 * material.high_frequency_gain()
    } else {
        1.0
    };
    let hf = ((-2.20 * bend - 0.28 * excess).exp()
        * (1.0 - 0.45 * wedge_sharpness).clamp(0.35, 1.0)
        * material_hf)
        .clamp(0.0, 1.0);
    let low_pass_hz = (700.0 + 19_300.0 * hf.sqrt()).clamp(80.0, 20_000.0);

    Some(
        AudioAcousticState {
            obstruction,
            occlusion,
            transmission_gain: broadband,
            high_frequency_gain: hf,
            low_pass_hz,
        }
        .sanitized(),
    )
}

fn resolve_effective_acoustic_route(
    settings: AudioOcclusionSettings,
    occlusion_observation: Option<&AudioOcclusionObservation>,
    diffraction_observation: Option<&AudioEdgeDiffractionObservation>,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    current_fixed_tick: u64,
) -> EffectiveAcousticRoute {
    let settings = settings.sanitized();
    if !settings.enabled {
        return EffectiveAcousticRoute::clear();
    }
    let Some(occlusion) = occlusion_observation.filter(|observation| {
        current_fixed_tick.saturating_sub(observation.fixed_tick)
            <= AUDIO_OCCLUSION_STALE_FIXED_TICKS
    }) else {
        return EffectiveAcousticRoute::clear();
    };

    let wall = settings.acoustic_state_with_material(
        occlusion.obstruction,
        occlusion.occlusion,
        occlusion.material,
    );
    let wall_route = EffectiveAcousticRoute {
        acoustic: wall,
        detour_delay_ms: 0.0,
        used_diffraction: false,
    };
    let Some(blocker) = occlusion.dominant_blocker_entity else {
        return wall_route;
    };
    let Some(diffraction) = diffraction_observation.filter(|observation| {
        current_fixed_tick.saturating_sub(observation.fixed_tick)
            <= AUDIO_DIFFRACTION_STALE_FIXED_TICKS
            && observation.blocker_entity == Some(blocker)
            && array_distance(observation.source_position, source_position)
                <= AUDIO_DIFFRACTION_POSITION_EPSILON_M
            && array_distance(observation.listener_position, listener_position)
                <= AUDIO_DIFFRACTION_POSITION_EPSILON_M
    }) else {
        return wall_route;
    };

    let mut best: Option<(AudioAcousticState, f32, [u32; 2])> = None;
    for path in &diffraction.paths {
        let Some(acoustic) = diffraction_path_acoustic_state(
            path,
            source_position,
            listener_position,
            occlusion.obstruction,
            occlusion.occlusion,
        ) else {
            continue;
        };
        let delay_ms =
            (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 500.0);
        let replace = best
            .as_ref()
            .is_none_or(|(current, current_delay, current_edge)| {
                acoustic
                    .transmission_gain
                    .total_cmp(&current.transmission_gain)
                    .then_with(|| {
                        acoustic
                            .high_frequency_gain
                            .total_cmp(&current.high_frequency_gain)
                    })
                    .then_with(|| current_delay.total_cmp(&delay_ms))
                    .then_with(|| current_edge.cmp(&path.edge_vertex_indices).reverse())
                    .is_gt()
            });
        if replace {
            best = Some((acoustic, delay_ms, path.edge_vertex_indices));
        }
    }
    let Some((edge, delay_ms, _)) = best else {
        return wall_route;
    };
    if edge.transmission_gain <= wall.transmission_gain + 1.0e-4 {
        return wall_route;
    }
    EffectiveAcousticRoute {
        acoustic: edge,
        detour_delay_ms: delay_ms,
        used_diffraction: true,
    }
}

#[inline]
fn environment_with_indirect_occlusion(
    environment: AudioEnvironmentState,
    acoustic: AudioAcousticState,
) -> AudioEnvironmentState {
    let mut environment = environment.sanitized();
    let acoustic = acoustic.sanitized();
    if !environment.is_wet() || (acoustic.obstruction <= 1.0e-4 && acoustic.occlusion <= 1.0e-4) {
        return environment;
    }
    // Energy removed from the direct path does not simply disappear indoors. A bounded part of
    // it reaches the listener as diffuse/early room energy. Keep the authored room preset and
    // change only send strength; geometry still controls the direct transmission separately.
    let blocked_energy = (1.0 - acoustic.transmission_gain).clamp(0.0, 1.0);
    let diffuse =
        blocked_energy * (acoustic.obstruction * 0.12 + acoustic.occlusion * 0.28).clamp(0.0, 0.40);
    environment.source_send.gain = (environment.source_send.gain + diffuse).clamp(0.0, 2.0);
    environment.listener_send.gain =
        (environment.listener_send.gain + diffuse * 0.65).clamp(0.0, 2.0);
    environment.sanitized()
}

fn environment_with_effective_direct_route(
    environment: AudioEnvironmentState,
    acoustic: AudioAcousticState,
    detour_delay_ms: f32,
) -> AudioEnvironmentState {
    let mut environment = environment_with_indirect_occlusion(environment, acoustic);
    let detour_delay_ms = if detour_delay_ms.is_finite() {
        detour_delay_ms.clamp(0.0, 500.0)
    } else {
        0.0
    };
    environment.direct_path.extra_delay_ms =
        (environment.direct_path.extra_delay_ms + detour_delay_ms).clamp(0.0, 500.0);
    environment.sanitized()
}

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

        let acoustic_route_by_key = emitters
            .iter()
            .map(|snapshot| {
                (
                    snapshot.stable_key,
                    self.update_acoustic_state(snapshot, dt, current_fixed_tick),
                )
            })
            .collect::<HashMap<_, _>>();
        let acoustic_by_key = acoustic_route_by_key
            .iter()
            .map(|(stable_key, route)| (*stable_key, route.acoustic))
            .collect::<HashMap<_, _>>();

        let environment_resolution_by_key = emitters
            .iter()
            .map(|snapshot| {
                (
                    snapshot.stable_key,
                    environment_frame.resolve_for_emitter(snapshot.stable_key, snapshot.position),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut environment_by_key = HashMap::with_capacity(emitters.len());
        for snapshot in &emitters {
            let resolution = environment_resolution_by_key
                .get(&snapshot.stable_key)
                .cloned()
                .unwrap_or_default();
            let acoustic = acoustic_by_key
                .get(&snapshot.stable_key)
                .copied()
                .unwrap_or_else(AudioAcousticState::clear);
            let detour_delay_ms = acoustic_route_by_key
                .get(&snapshot.stable_key)
                .map(|route| route.detour_delay_ms)
                .unwrap_or(0.0);
            let state =
                self.update_environment_state(snapshot, &resolution, acoustic, detour_delay_ms, dt);
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
                emitter_environment: environment_resolution.emitter_zone,
                listener_environment: environment_resolution.listener_zone,
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
        let mut emitter = AudioEmitter {
            gain: f32::INFINITY,
            ..AudioEmitter::default()
        };
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

    fn test_material(transmission_gain: f32) -> newengine_audio_api::AcousticMaterialProfile {
        newengine_audio_api::AcousticMaterialProfile {
            transmission_gain,
            reflection_gain: 0.82,
            high_frequency_absorption: 0.24,
            low_pass_hz: 8_500.0,
        }
    }

    fn blocked_observation(
        fixed_tick: u64,
        blocker: u64,
        material: newengine_audio_api::AcousticMaterialProfile,
    ) -> AudioOcclusionObservation {
        AudioOcclusionObservation {
            fixed_tick,
            samples: 3,
            blocked_samples: 3,
            obstruction: 1.0,
            occlusion: 1.0,
            estimated_thickness_m: 1.0,
            center_blocker_layers: 1,
            dominant_blocker_entity: Some(blocker),
            dominant_material: "surface.test".to_owned(),
            material,
        }
    }

    fn diffraction_path(
        visible: bool,
        excess_length_m: f32,
        bend_angle_radians: f32,
        material: newengine_audio_api::AcousticMaterialProfile,
    ) -> AudioEdgeDiffractionPathObservation {
        AudioEdgeDiffractionPathObservation {
            edge_vertex_indices: [2, 7],
            visible,
            diffraction_point: [2.0, 1.0, 0.0],
            arrival_direction: [0.0, 1.0, 0.0],
            path_length_m: 4.0 + excess_length_m,
            excess_length_m,
            bend_angle_radians,
            wedge_angle_radians: std::f32::consts::FRAC_PI_2,
            material_known: true,
            material,
        }
    }

    fn diffraction_observation(
        fixed_tick: u64,
        blocker: u64,
        path: AudioEdgeDiffractionPathObservation,
    ) -> AudioEdgeDiffractionObservation {
        AudioEdgeDiffractionObservation {
            fixed_tick,
            source_position: [4.0, 0.0, 0.0],
            listener_position: [0.0, 0.0, 0.0],
            blocker_entity: Some(blocker),
            paths: vec![path],
        }
    }

    #[test]
    fn stale_diffraction_falls_back_to_through_wall_route() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(20, 77, test_material(0.08));
        let edge = diffraction_observation(
            10,
            77,
            diffraction_path(true, 0.4, 0.5, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            20,
        );
        let expected = settings.acoustic_state_with_material(1.0, 1.0, wall.material);
        assert!(!route.used_diffraction);
        assert_eq!(route.detour_delay_ms, 0.0);
        assert_eq!(route.acoustic, expected);
    }

    #[test]
    fn blocker_mismatch_rejects_unrelated_scene_edge() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(30, 77, test_material(0.08));
        let edge = diffraction_observation(
            30,
            88,
            diffraction_path(true, 0.3, 0.4, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            30,
        );
        assert!(!route.used_diffraction);
        assert_eq!(route.detour_delay_ms, 0.0);
    }

    #[test]
    fn blocked_edge_visibility_cannot_bypass_wall() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(40, 77, test_material(0.05));
        let edge = diffraction_observation(
            40,
            77,
            diffraction_path(false, 0.2, 0.25, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            40,
        );
        assert!(!route.used_diffraction);
    }

    #[test]
    fn strongest_route_selects_diffraction_or_wall_by_broadband_energy() {
        let edge = diffraction_observation(
            50,
            77,
            diffraction_path(true, 0.3, 0.35, test_material(0.95)),
        );
        let heavy_wall = blocked_observation(50, 77, test_material(0.04));
        let edge_route = resolve_effective_acoustic_route(
            AudioOcclusionSettings::default(),
            Some(&heavy_wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            50,
        );
        assert!(edge_route.used_diffraction);
        assert!(edge_route.detour_delay_ms > 0.0);

        let permissive_settings = AudioOcclusionSettings {
            obstruction_gain: 0.97,
            occlusion_gain: 0.97,
            ..AudioOcclusionSettings::default()
        };
        let light_wall = blocked_observation(
            50,
            77,
            newengine_audio_api::AcousticMaterialProfile::transparent(),
        );
        let wall_route = resolve_effective_acoustic_route(
            permissive_settings,
            Some(&light_wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            50,
        );
        assert!(!wall_route.used_diffraction);
        assert_eq!(wall_route.detour_delay_ms, 0.0);
    }

    #[test]
    fn diffraction_hf_loss_increases_with_bend_and_excess_distance() {
        let material = test_material(0.5);
        let mild = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.10, 0.20, material),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("mild edge");
        let severe = diffraction_path_acoustic_state(
            &diffraction_path(true, 1.20, 1.40, material),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("severe edge");
        assert!(severe.transmission_gain < mild.transmission_gain);
        assert!(severe.high_frequency_gain < mild.high_frequency_gain);
        assert!(severe.low_pass_hz < mild.low_pass_hz);
    }

    #[test]
    fn diffraction_route_never_reuses_wall_transmission_gain() {
        let opaque = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.4, 0.6, test_material(0.05)),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("opaque material edge");
        let transmissive = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.4, 0.6, test_material(0.95)),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("transmissive material edge");
        assert_eq!(opaque, transmissive);
    }

    #[test]
    fn diffraction_detour_does_not_double_apply_same_room_attenuation() {
        let clear = AudioEnvironmentState::clear();
        let acoustic = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.55,
            high_frequency_gain: 0.42,
            low_pass_hz: 5_500.0,
        };
        let routed = environment_with_effective_direct_route(clear, acoustic, 12.5);
        assert_eq!(routed.direct_path.gain, 1.0);
        assert_eq!(routed.direct_path.high_frequency_gain, 1.0);
        assert_eq!(routed.direct_path.low_pass_hz, 20_000.0);
        assert!((routed.direct_path.extra_delay_ms - 12.5).abs() < 1.0e-6);
    }

    #[test]
    fn occlusion_redirects_some_direct_energy_into_existing_room_tail() {
        let dry = AudioEnvironmentState::clear();
        let blocked = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.2,
            high_frequency_gain: 0.3,
            low_pass_hz: 1_500.0,
        };
        assert_eq!(environment_with_indirect_occlusion(dry, blocked), dry);

        let room = AudioEnvironmentState {
            source_send: newengine_audio_api::AudioReverbSend {
                room_bus_id: 0,
                gain: 0.30,
                preset: newengine_audio_api::AudioReverbPreset::room(),
                early_reflections: newengine_audio_api::AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            listener_send: newengine_audio_api::AudioReverbSend {
                room_bus_id: 0,
                gain: 0.20,
                preset: newengine_audio_api::AudioReverbPreset::room(),
                early_reflections: newengine_audio_api::AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: newengine_audio_api::AudioDirectPathResponse::clear(),
            portal_gain: 1.0,
        };
        let indirect = environment_with_indirect_occlusion(room, blocked);
        assert!(indirect.source_send.gain > room.source_send.gain);
        assert!(indirect.listener_send.gain > room.listener_send.gain);
        assert_eq!(indirect.portal_gain, room.portal_gain);
    }
}
