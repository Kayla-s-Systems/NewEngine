#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{HashMap, HashSet};

use newengine_audio_api::{
    AudioAcousticState, AudioAmbienceBed, AudioAmbienceScope, AudioBus, AudioSpatialParams,
    AudioStreamPlayRequest, AudioVoiceUpdateRequest,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ecs::EntityId;
use newengine_transform::{read_entity_world_pose_local_chain, Transform};

use crate::audio_environment::AudioEnvironmentFrame;
use crate::AudioWorldScene;
use newengine_audio_client::{
    audio_service_info, play_audio_stream, stop_audio_voice, update_audio_voice,
};
use newengine_audio_world_api::AudioAmbienceBedRuntime;

const AMBIENCE_ACTIVE_EPSILON: f32 = 1.0e-4;
const AMBIENCE_RETRY_TICKS: u64 = 60;

#[derive(Clone, Debug)]
struct BedSnapshot {
    entity: EntityId,
    stable_key: u64,
    bed: AudioAmbienceBed,
    position: [f32; 3],
}

#[derive(Clone, Debug)]
struct ManagedBed {
    voice_id: u64,
    stream: String,
}

pub struct AudioAmbienceRuntimeModule {
    scene: AudioWorldScene,
    managed: HashMap<u64, ManagedBed>,
    gains: HashMap<u64, f32>,
    retry_after_tick: HashMap<u64, u64>,
    provider: Option<String>,
    services_generation: u64,
    tick: u64,
}

impl AudioAmbienceRuntimeModule {
    pub fn new(scene: AudioWorldScene) -> Self {
        Self {
            scene,
            managed: HashMap::new(),
            gains: HashMap::new(),
            retry_after_tick: HashMap::new(),
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
                self.managed.clear();
                self.retry_after_tick.clear();
                self.provider = next_provider;
            }
        }
        self.provider.is_some()
    }

    fn snapshot(&self) -> (AudioEnvironmentFrame, Vec<BedSnapshot>) {
        let scene = self.scene.scene();
        let scene = scene.read();
        let world = scene.world();
        let environment = AudioEnvironmentFrame::snapshot(world);
        let mut beds = Vec::new();
        for entity in world.iter_entities() {
            let Some(bed) = world.get::<AudioAmbienceBed>(entity).cloned() else {
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
            beds.push(BedSnapshot {
                entity,
                stable_key: entity.stable_u64(),
                bed: bed.sanitized(),
                position,
            });
        }
        beds.sort_by_key(|snapshot| snapshot.stable_key);
        (environment, beds)
    }

    pub fn target_activation(bed: &AudioAmbienceBed, environment: &AudioEnvironmentFrame) -> f32 {
        if !bed.enabled || bed.stream.uri.is_empty() || !environment.listener_ready() {
            return 0.0;
        }
        match bed.scope {
            AudioAmbienceScope::Global => 1.0,
            AudioAmbienceScope::Indoor => {
                if environment.listener_is_outdoor() {
                    0.0
                } else {
                    1.0
                }
            }
            AudioAmbienceScope::Outdoor => {
                if environment.listener_is_outdoor() {
                    1.0
                } else {
                    0.0
                }
            }
            AudioAmbienceScope::Zones => {
                let listener_zone = environment.listener_zone_id().unwrap_or_default();
                if bed.zones.iter().any(|zone| zone == listener_zone) {
                    return 1.0;
                }
                bed.zones
                    .iter()
                    .map(|zone| environment.route_gain_to_zone(zone))
                    .fold(0.0_f32, f32::max)
                    * bed.portal_bleed
            }
        }
        .clamp(0.0, 1.0)
    }

    fn smoothed_gain(current: f32, target: f32, dt: f32, fade_seconds: f32) -> f32 {
        let current = if current.is_finite() {
            current.max(0.0)
        } else {
            0.0
        };
        let target = if target.is_finite() {
            target.max(0.0)
        } else {
            0.0
        };
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            1.0 / 60.0
        };
        let fade = if fade_seconds.is_finite() {
            fade_seconds.clamp(0.02, 30.0)
        } else {
            1.5
        };
        if dt <= 0.0 {
            return current;
        }
        let alpha = 1.0 - (-dt / fade).exp();
        current + (target - current) * alpha
    }

    fn stop_managed(&mut self, stable_key: u64) {
        if let Some(managed) = self.managed.remove(&stable_key) {
            let _ = stop_audio_voice(managed.voice_id);
        }
    }

    fn update_beds(&mut self, dt: f32) {
        self.tick = self.tick.wrapping_add(1);
        let playback_available = self.refresh_provider();
        let (environment, beds) = self.snapshot();
        let live = beds
            .iter()
            .map(|bed| bed.stable_key)
            .collect::<HashSet<_>>();

        let stale = self
            .managed
            .keys()
            .copied()
            .filter(|key| !live.contains(key))
            .collect::<Vec<_>>();
        for key in stale {
            self.stop_managed(key);
            self.gains.remove(&key);
            self.retry_after_tick.remove(&key);
        }

        for snapshot in &beds {
            let target = Self::target_activation(&snapshot.bed, &environment) * snapshot.bed.gain;
            let current = self.gains.get(&snapshot.stable_key).copied().unwrap_or(0.0);
            let next = Self::smoothed_gain(current, target, dt, snapshot.bed.fade_seconds);
            self.gains.insert(snapshot.stable_key, next);

            let environment_state = if snapshot.bed.spatial {
                environment.resolve(snapshot.position).state
            } else {
                environment.listener_environment_state()
            };

            if self
                .managed
                .get(&snapshot.stable_key)
                .is_some_and(|managed| managed.stream != snapshot.bed.stream.uri)
            {
                self.stop_managed(snapshot.stable_key);
            }

            if let Some(managed) = self.managed.get(&snapshot.stable_key).cloned() {
                if target <= AMBIENCE_ACTIVE_EPSILON && next <= AMBIENCE_ACTIVE_EPSILON {
                    self.stop_managed(snapshot.stable_key);
                    continue;
                }
                let request = AudioVoiceUpdateRequest {
                    voice_id: managed.voice_id,
                    gain: Some(next),
                    speed: None,
                    seek_seconds: None,
                    paused: Some(false),
                    position: snapshot.bed.spatial.then_some(snapshot.position),
                    acoustic: Some(AudioAcousticState::clear()),
                    environment: Some(environment_state),
                };
                match update_audio_voice(&request) {
                    Ok(Some(ack)) if ack.accepted => {}
                    Ok(_) | Err(_) => {
                        self.managed.remove(&snapshot.stable_key);
                        self.retry_after_tick.insert(
                            snapshot.stable_key,
                            self.tick.saturating_add(AMBIENCE_RETRY_TICKS),
                        );
                    }
                }
                continue;
            }

            if !playback_available
                || target <= AMBIENCE_ACTIVE_EPSILON
                || self
                    .retry_after_tick
                    .get(&snapshot.stable_key)
                    .is_some_and(|retry| self.tick < *retry)
            {
                continue;
            }

            let mut request = AudioStreamPlayRequest::new(snapshot.bed.stream.uri.clone());
            request.bus = AudioBus::Ambience;
            request.gain = next.max(AMBIENCE_ACTIVE_EPSILON * 2.0);
            request.looping = snapshot.bed.looping;
            request.spatial = snapshot.bed.spatial.then_some(AudioSpatialParams {
                position: snapshot.position,
            });
            request.attenuation = snapshot.bed.attenuation.clone();
            request.environment = environment_state;
            request.buffer = snapshot.bed.buffer;
            request.priority = snapshot.bed.priority;
            request.concurrency_group =
                format!("ambience.{}.{}", snapshot.bed.bed_id, snapshot.stable_key);

            match play_audio_stream(&request) {
                Ok(Some(ack)) if ack.accepted => {
                    if let Some(voice_id) = ack.voice_id {
                        self.managed.insert(
                            snapshot.stable_key,
                            ManagedBed {
                                voice_id,
                                stream: snapshot.bed.stream.uri.clone(),
                            },
                        );
                        self.retry_after_tick.remove(&snapshot.stable_key);
                    }
                }
                Ok(_) | Err(_) => {
                    self.retry_after_tick.insert(
                        snapshot.stable_key,
                        self.tick.saturating_add(AMBIENCE_RETRY_TICKS),
                    );
                }
            }
        }

        self.publish_runtime(&environment, &beds);
    }

    fn publish_runtime(&self, environment: &AudioEnvironmentFrame, beds: &[BedSnapshot]) {
        let provider = self.provider.clone().unwrap_or_default();
        let listener_zone = environment
            .listener_zone_id()
            .unwrap_or_default()
            .to_owned();
        let listener_outdoor = environment.listener_is_outdoor();
        let scene = self.scene.scene();
        let mut scene = scene.write();
        let world = scene.world_mut();
        for snapshot in beds {
            if !world.exists(snapshot.entity) {
                continue;
            }
            let current_gain = self.gains.get(&snapshot.stable_key).copied().unwrap_or(0.0);
            let target_gain =
                Self::target_activation(&snapshot.bed, environment) * snapshot.bed.gain;
            let portal_gain = match snapshot.bed.scope {
                AudioAmbienceScope::Zones => snapshot
                    .bed
                    .zones
                    .iter()
                    .map(|zone| environment.route_gain_to_zone(zone))
                    .fold(0.0_f32, f32::max),
                _ => 1.0,
            };
            let runtime = AudioAmbienceBedRuntime {
                voice_id: self
                    .managed
                    .get(&snapshot.stable_key)
                    .map(|managed| managed.voice_id),
                bed_id: snapshot.bed.bed_id.clone(),
                stream: snapshot.bed.stream.uri.clone(),
                current_gain,
                target_gain,
                listener_zone: listener_zone.clone(),
                listener_outdoor,
                portal_gain,
                provider: provider.clone(),
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
        self.gains.clear();
        self.retry_after_tick.clear();
    }
}

impl<E: Send + 'static> Module<E> for AudioAmbienceRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.audio.ambience-runtime"
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let dt = ctx.frame().map(|frame| frame.dt).unwrap_or(1.0 / 60.0);
        self.update_beds(dt);
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

    fn insert_zone(
        world: &mut newengine_ecs::World,
        id: &str,
        x: f32,
        kind: newengine_audio_api::AudioEnvironmentKind,
    ) {
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            newengine_transform::Transform {
                position: newengine_math::Vec3::new(x, 0.0, 0.0),
                ..Default::default()
            },
        );
        let _ = world.insert(
            entity,
            newengine_audio_api::AudioEnvironmentZone {
                zone_id: id.to_owned(),
                kind,
                half_extents: [5.0, 5.0, 5.0],
                blend_distance: 0.0,
                ..Default::default()
            },
        );
    }

    #[test]
    fn ambience_scope_tracks_indoor_outdoor_and_portal_zone_routes() {
        let mut world = newengine_ecs::World::new();
        insert_zone(
            &mut world,
            "room.indoor",
            0.0,
            newengine_audio_api::AudioEnvironmentKind::Indoor,
        );
        insert_zone(
            &mut world,
            "yard.outdoor",
            20.0,
            newengine_audio_api::AudioEnvironmentKind::Outdoor,
        );
        let portal_entity = world.spawn();
        let mut portal = newengine_audio_api::AudioPortal::new(
            "door.indoor-yard",
            "room.indoor",
            "yard.outdoor",
        );
        portal.openness = 0.5;
        let _ = world.insert(portal_entity, portal);

        let indoor_frame = AudioEnvironmentFrame::snapshot_at(&world, newengine_math::Vec3::ZERO);
        let mut indoor = AudioAmbienceBed::new("inside", "shared/audio/inside.ogg");
        indoor.scope = AudioAmbienceScope::Indoor;
        assert_eq!(
            AudioAmbienceRuntimeModule::target_activation(&indoor, &indoor_frame),
            1.0
        );

        let mut outdoor = AudioAmbienceBed::new("outside", "shared/audio/outside.ogg");
        outdoor.scope = AudioAmbienceScope::Outdoor;
        assert_eq!(
            AudioAmbienceRuntimeModule::target_activation(&outdoor, &indoor_frame),
            0.0
        );

        let mut zone_bed = AudioAmbienceBed::new("yard", "shared/audio/yard.ogg");
        zone_bed.scope = AudioAmbienceScope::Zones;
        zone_bed.zones = vec!["yard.outdoor".to_owned()];
        zone_bed.portal_bleed = 0.4;
        let activation = AudioAmbienceRuntimeModule::target_activation(&zone_bed, &indoor_frame);
        assert!((activation - 0.2).abs() < 1.0e-6);

        let outdoor_frame =
            AudioEnvironmentFrame::snapshot_at(&world, newengine_math::Vec3::new(20.0, 0.0, 0.0));
        assert_eq!(
            AudioAmbienceRuntimeModule::target_activation(&outdoor, &outdoor_frame),
            1.0
        );
        assert_eq!(
            AudioAmbienceRuntimeModule::target_activation(&zone_bed, &outdoor_frame),
            1.0
        );
    }

    #[test]
    fn ambience_gain_fades_without_overshoot() {
        let mut gain = 0.0;
        for _ in 0..120 {
            gain = AudioAmbienceRuntimeModule::smoothed_gain(gain, 1.0, 1.0 / 60.0, 1.0);
        }
        assert!(gain > 0.8 && gain < 1.0);
        let faded = AudioAmbienceRuntimeModule::smoothed_gain(gain, 0.0, 1.0 / 60.0, 1.0);
        assert!(faded < gain && faded >= 0.0);
    }
}
