#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use newengine_audio_api::{
    AcousticMaterialProfile, AcousticSurface, AudioEmitter, AudioListenerState,
    AudioOcclusionSettings,
};
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_transform::{read_entity_world_pose_local_chain, Transform};
use parking_lot::Mutex;

use crate::gameplay::{first_player, GameplayPhysicsQueryProvider, PhysicsSurface};

const AUDIO_OCCLUSION_QUERY_NAMESPACE: u64 = 0xa0c0_0000_0000_0000;
const AUDIO_OCCLUSION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const DEFAULT_MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 32;
const MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 256;
const ENDPOINT_EPSILON: f32 = 0.025;

/// Last presentation-cadence listener pose projected into ECS for fixed-step
/// acoustic physics probes. A fixed tick may observe the previous presentation
/// frame, which is intentional and avoids coupling the physics service to camera
/// implementation details.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioListenerRuntimeState {
    pub listener: AudioListenerState,
    pub frame_index: u64,
}

/// Raw fixed-step visibility observation. This is deliberately not the final
/// audible state: `AudioSceneRuntimeModule` smooths it before crossing the
/// `engine.audio` boundary.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioOcclusionObservation {
    pub fixed_tick: u64,
    pub samples: u8,
    pub blocked_samples: u8,
    pub obstruction: f32,
    pub occlusion: f32,
    pub dominant_material: String,
    pub material: AcousticMaterialProfile,
}

#[derive(Clone, Copy, Debug)]
struct PendingProbeRay {
    emitter_key: u64,
    sample_index: u8,
    max_t: f32,
}

#[derive(Clone, Copy, Debug)]
struct AcousticCandidate {
    stable_key: u64,
    position: Vec3,
    distance: f32,
    settings: AudioOcclusionSettings,
}

#[derive(Clone, Debug, Default)]
struct ProbeAggregate {
    sample_count: u8,
    blocked: u8,
    center_blocked: bool,
    transmission_sum: f32,
    absorption_sum: f32,
    low_pass_sum: f32,
    materials: BTreeMap<String, u8>,
}

/// Stable audio-domain material presets keyed from gameplay `PhysicsSurface.id`.
/// Projects may use more specific ids; matching is intentionally suffix/substring based
/// so `surface.wall.concrete` and `surface.concrete` share the same first-party profile.
pub fn acoustic_material_profile_for_surface(surface_id: &str) -> AcousticMaterialProfile {
    let id = surface_id.trim().to_ascii_lowercase();
    let profile = if id.contains("concrete") || id.contains("stone") || id.contains("brick") {
        AcousticMaterialProfile {
            transmission_gain: 0.16,
            high_frequency_absorption: 0.92,
            low_pass_hz: 1_100.0,
        }
    } else if id.contains("glass") {
        AcousticMaterialProfile {
            transmission_gain: 0.58,
            high_frequency_absorption: 0.42,
            low_pass_hz: 6_500.0,
        }
    } else if id.contains("wood") || id.contains("timber") {
        AcousticMaterialProfile {
            transmission_gain: 0.36,
            high_frequency_absorption: 0.72,
            low_pass_hz: 2_800.0,
        }
    } else if id.contains("metal") || id.contains("steel") {
        AcousticMaterialProfile {
            transmission_gain: 0.12,
            high_frequency_absorption: 0.84,
            low_pass_hz: 1_700.0,
        }
    } else if id.contains("dirt") || id.contains("soil") || id.contains("earth") {
        AcousticMaterialProfile {
            transmission_gain: 0.24,
            high_frequency_absorption: 0.86,
            low_pass_hz: 1_900.0,
        }
    } else {
        AcousticMaterialProfile::default()
    };
    profile.sanitized()
}

/// Engine-owned physics-query contributor for spatial audio obstruction.
/// It emits provider-neutral `PhysicsQueryDto` batches only; Jolt/Bullet details
/// remain entirely behind the stable `engine.physics` transport.
pub struct AudioOcclusionPhysicsQueryProvider {
    pending: Mutex<BTreeMap<u64, PendingProbeRay>>,
    next_query: AtomicU64,
    max_emitters_per_tick: usize,
}

impl Default for AudioOcclusionPhysicsQueryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOcclusionPhysicsQueryProvider {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(BTreeMap::new()),
            next_query: AtomicU64::new(1),
            max_emitters_per_tick: occlusion_emitter_budget_from_env(),
        }
    }

    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    #[cfg(test)]
    fn with_emitter_budget(max_emitters_per_tick: usize) -> Self {
        Self {
            max_emitters_per_tick: max_emitters_per_tick.clamp(1, MAX_OCCLUSION_EMITTERS_PER_TICK),
            ..Self::new()
        }
    }

    #[inline]
    fn alloc_query_seq(&self) -> u64 {
        let value =
            self.next_query.fetch_add(1, Ordering::Relaxed) & AUDIO_OCCLUSION_QUERY_COUNTER_MASK;
        AUDIO_OCCLUSION_QUERY_NAMESPACE | value.max(1)
    }

    fn collect_candidates(&self, world: &World, listener: Vec3) -> Vec<AcousticCandidate> {
        let mut candidates = Vec::new();
        for entity in world.iter_entities() {
            let Some(emitter) = world.get::<AudioEmitter>(entity) else {
                continue;
            };
            let settings = emitter.occlusion.sanitized();
            if !emitter.enabled
                || !emitter.spatial
                || !settings.enabled
                || emitter.cue.trim().is_empty()
            {
                continue;
            }
            let position = read_entity_world_pose_local_chain(world, entity)
                .map(|pose| pose.0)
                .or_else(|| {
                    world
                        .get::<Transform>(entity)
                        .map(|transform| transform.position)
                })
                .unwrap_or(Vec3::ZERO);
            if !position.is_finite() {
                continue;
            }
            let distance = (position - listener).length();
            if !distance.is_finite() || distance <= 1.0e-4 || distance > settings.max_distance {
                continue;
            }
            candidates.push(AcousticCandidate {
                stable_key: entity.stable_u64(),
                position,
                distance,
                settings,
            });
        }
        candidates.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| a.stable_key.cmp(&b.stable_key))
        });
        candidates.truncate(self.max_emitters_per_tick);
        candidates
    }

    fn build_probe_rays(
        &self,
        listener: Vec3,
        candidate: AcousticCandidate,
        ignore_entity: Option<u64>,
        pending: &mut BTreeMap<u64, PendingProbeRay>,
    ) -> Vec<PhysicsQueryDto> {
        let to_emitter = candidate.position - listener;
        let direction = to_emitter.normalize_or_zero();
        if direction.length_squared() <= 1.0e-8 {
            return Vec::new();
        }
        let right = stable_probe_right(direction);
        let up = right.cross(direction).normalize_or_zero();
        let radius = candidate.settings.probe_radius;
        let offsets = [
            Vec3::ZERO,
            right * radius,
            -right * radius,
            up * radius,
            -up * radius,
        ];
        let sample_count = candidate.settings.ray_count.clamp(1, 5);
        let mut queries = Vec::with_capacity(sample_count as usize);
        for sample_index in 0..sample_count {
            let target = candidate.position + offsets[sample_index as usize];
            let ray = target - listener;
            let max_t = ray.length();
            if !max_t.is_finite() || max_t <= 1.0e-4 {
                continue;
            }
            let dir = ray / max_t;
            let seq = self.alloc_query_seq();
            pending.insert(
                seq,
                PendingProbeRay {
                    emitter_key: candidate.stable_key,
                    sample_index,
                    max_t,
                },
            );
            queries.push(PhysicsQueryDto {
                seq,
                ignore_entity,
                kind: PhysicsQueryKindDto::Ray {
                    origin: vec3_array(listener),
                    dir: vec3_array(dir),
                    max_t,
                },
            });
        }
        queries
    }
}

impl GameplayPhysicsQueryProvider for AudioOcclusionPhysicsQueryProvider {
    fn id(&self) -> &'static str {
        "engine.audio.physics-occlusion"
    }

    fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto> {
        let Some(listener_state) = world.resource::<AudioListenerRuntimeState>().copied() else {
            self.pending.lock().clear();
            return Vec::new();
        };
        let listener_position = listener_state.listener.sanitized().position;
        let listener = Vec3::new(
            listener_position[0],
            listener_position[1],
            listener_position[2],
        );
        if !listener.is_finite() {
            self.pending.lock().clear();
            return Vec::new();
        }

        let ignore_entity = first_player(world).map(EntityId::stable_u64);
        let candidates = self.collect_candidates(world, listener);
        let mut pending = BTreeMap::new();
        let mut queries = Vec::new();
        for candidate in candidates {
            queries.extend(self.build_probe_rays(listener, candidate, ignore_entity, &mut pending));
        }
        *self.pending.lock() = pending;
        queries
    }

    fn resolve_query_hits(
        &self,
        world: &mut World,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64> {
        let pending = std::mem::take(&mut *self.pending.lock());
        if pending.is_empty() {
            return BTreeSet::new();
        }
        let hits_by_seq = hits
            .iter()
            .map(|hit| (hit.seq, *hit))
            .collect::<BTreeMap<_, _>>();
        let mut consumed = BTreeSet::new();
        let mut aggregates = BTreeMap::<u64, ProbeAggregate>::new();

        for (seq, probe) in pending {
            consumed.insert(seq);
            let blocker = hits_by_seq.get(&seq).filter(|hit| {
                hit.entity != probe.emitter_key
                    && hit.distance.is_finite()
                    && hit.distance >= 0.0
                    && hit.distance + ENDPOINT_EPSILON < probe.max_t
            });
            let aggregate = aggregates.entry(probe.emitter_key).or_default();
            aggregate.sample_count = aggregate.sample_count.saturating_add(1);
            if let Some(hit) = blocker {
                aggregate.blocked = aggregate.blocked.saturating_add(1);
                if probe.sample_index == 0 {
                    aggregate.center_blocked = true;
                }
                let blocker_entity = key_to_entity.get(&hit.entity).copied();
                let authored = blocker_entity
                    .and_then(|entity| world.get::<AcousticSurface>(entity))
                    .cloned()
                    .map(AcousticSurface::sanitized);
                let (material_id, material) = if let Some(authored) = authored {
                    (authored.material_id, authored.profile)
                } else {
                    let surface_id = blocker_entity
                        .and_then(|entity| world.get::<PhysicsSurface>(entity))
                        .map(|surface| surface.id.as_str())
                        .unwrap_or("surface.default");
                    (
                        surface_id.to_owned(),
                        acoustic_material_profile_for_surface(surface_id),
                    )
                };
                aggregate.transmission_sum += material.transmission_gain;
                aggregate.absorption_sum += material.high_frequency_absorption;
                aggregate.low_pass_sum += material.low_pass_hz;
                *aggregate.materials.entry(material_id).or_insert(0) += 1;
            }
        }

        for (emitter_key, aggregate) in aggregates {
            let samples = aggregate.sample_count.max(1);
            let blocked_samples = aggregate.blocked.min(samples);
            let obstruction = f32::from(blocked_samples) / f32::from(samples);
            let occlusion = if aggregate.center_blocked && blocked_samples == samples {
                1.0
            } else {
                0.0
            };
            let material = if blocked_samples > 0 {
                let count = f32::from(blocked_samples);
                AcousticMaterialProfile {
                    transmission_gain: aggregate.transmission_sum / count,
                    high_frequency_absorption: aggregate.absorption_sum / count,
                    low_pass_hz: aggregate.low_pass_sum / count,
                }
                .sanitized()
            } else {
                AcousticMaterialProfile {
                    transmission_gain: 1.0,
                    high_frequency_absorption: 0.0,
                    low_pass_hz: 20_000.0,
                }
            };
            let dominant_material = aggregate
                .materials
                .iter()
                .max_by(|(a_id, a_count), (b_id, b_count)| {
                    a_count.cmp(b_count).then_with(|| b_id.cmp(a_id))
                })
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| "surface.clear".to_owned());
            let Some(entity) = key_to_entity.get(&emitter_key).copied() else {
                continue;
            };
            if world.exists(entity) {
                let _ = world.insert(
                    entity,
                    AudioOcclusionObservation {
                        fixed_tick,
                        samples,
                        blocked_samples,
                        obstruction,
                        occlusion,
                        dominant_material,
                        material,
                    },
                );
            }
        }
        consumed
    }
}

#[inline]
fn stable_probe_right(direction: Vec3) -> Vec3 {
    let mut right = direction.cross(Vec3::Y);
    if right.length_squared() <= 1.0e-8 {
        right = direction.cross(Vec3::X);
    }
    right.normalize_or_zero()
}

#[inline]
fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn occlusion_emitter_budget_from_env() -> usize {
    crate::env_config::var("NEWENGINE_AUDIO_OCCLUSION_MAX_EMITTERS_PER_TICK")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_OCCLUSION_EMITTERS_PER_TICK))
        .unwrap_or(DEFAULT_MAX_OCCLUSION_EMITTERS_PER_TICK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listener_world() -> World {
        let mut world = World::new();
        world.insert_resource(AudioListenerRuntimeState::default());
        world
    }

    #[test]
    fn provider_emits_bounded_multi_ray_batch_for_nearest_spatial_emitters() {
        let mut world = listener_world();
        for index in 0..3 {
            let entity = world.spawn();
            let _ = world.insert(
                entity,
                Transform {
                    position: Vec3::new(0.0, 0.0, -(5.0 + index as f32 * 5.0)),
                    ..Transform::default()
                },
            );
            let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
            emitter.occlusion.ray_count = 3;
            let _ = world.insert(entity, emitter);
        }
        let provider = AudioOcclusionPhysicsQueryProvider::with_emitter_budget(2);
        let queries = provider.collect_queries(&world);
        assert_eq!(queries.len(), 6);
        assert!(queries
            .iter()
            .all(|query| query.seq & 0xfff0_0000_0000_0000 == AUDIO_OCCLUSION_QUERY_NAMESPACE));
    }

    #[test]
    fn partial_probe_blockage_is_obstruction_not_full_occlusion() {
        let mut world = listener_world();
        let emitter_entity = world.spawn();
        let _ = world.insert(
            emitter_entity,
            Transform {
                position: Vec3::new(0.0, 0.0, -10.0),
                ..Transform::default()
            },
        );
        let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
        emitter.occlusion.ray_count = 3;
        let _ = world.insert(emitter_entity, emitter);
        let blocker = world.spawn();

        let provider = AudioOcclusionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        assert_eq!(queries.len(), 3);
        let hit = PhysicsQueryHitDto {
            seq: queries[0].seq,
            entity: blocker.stable_u64(),
            position: [0.0, 0.0, -4.0],
            normal: [0.0, 0.0, 1.0],
            distance: 4.0,
        };
        let keys = world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect::<BTreeMap<_, _>>();
        let consumed = provider.resolve_query_hits(&mut world, 7, &[hit], &keys);
        assert_eq!(consumed.len(), 3);
        let observation = world
            .get::<AudioOcclusionObservation>(emitter_entity)
            .cloned()
            .expect("observation");
        assert_eq!(observation.samples, 3);
        assert_eq!(observation.blocked_samples, 1);
        assert!((observation.obstruction - 1.0 / 3.0).abs() < 1.0e-6);
        assert_eq!(observation.occlusion, 0.0);
    }

    #[test]
    fn all_probe_rays_blocked_produces_full_occlusion() {
        let mut world = listener_world();
        let emitter_entity = world.spawn();
        let _ = world.insert(
            emitter_entity,
            Transform {
                position: Vec3::new(0.0, 0.0, -10.0),
                ..Transform::default()
            },
        );
        let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
        emitter.occlusion.ray_count = 3;
        let _ = world.insert(emitter_entity, emitter);
        let blocker = world.spawn();

        let provider = AudioOcclusionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let hits = queries
            .iter()
            .map(|query| PhysicsQueryHitDto {
                seq: query.seq,
                entity: blocker.stable_u64(),
                position: [0.0, 0.0, -4.0],
                normal: [0.0, 0.0, 1.0],
                distance: 4.0,
            })
            .collect::<Vec<_>>();
        let keys = world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect::<BTreeMap<_, _>>();
        provider.resolve_query_hits(&mut world, 9, &hits, &keys);
        let observation = world
            .get::<AudioOcclusionObservation>(emitter_entity)
            .cloned()
            .expect("observation");
        assert_eq!(observation.blocked_samples, 3);
        assert_eq!(observation.obstruction, 1.0);
        assert_eq!(observation.occlusion, 1.0);
    }
    #[test]
    fn acoustic_surface_presets_are_spectrally_distinct() {
        let concrete = acoustic_material_profile_for_surface("surface.wall.concrete");
        let glass = acoustic_material_profile_for_surface("surface.glass");
        let wood = acoustic_material_profile_for_surface("surface.wood");
        let metal = acoustic_material_profile_for_surface("surface.metal");
        assert!(concrete.low_pass_hz < glass.low_pass_hz);
        assert!(concrete.high_frequency_absorption > glass.high_frequency_absorption);
        assert!(metal.transmission_gain < wood.transmission_gain);
    }
}
