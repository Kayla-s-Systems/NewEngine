#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use newengine_audio_api::{
    AcousticMaterialLibrary, AcousticMaterialProfile, AcousticSurface, AudioEmitter,
    AudioOcclusionSettings,
};
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_transform::{read_entity_world_pose_local_chain, Transform};
use parking_lot::Mutex;

use newengine_audio_world_api::{AudioListenerRuntimeState, AudioOcclusionObservation};

use crate::gameplay::{first_player, GameplayPhysicsQueryProvider, PhysicsSurface};

const AUDIO_OCCLUSION_QUERY_NAMESPACE: u64 = 0xa0c0_0000_0000_0000;
const AUDIO_OCCLUSION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const DEFAULT_MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 32;
const MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 256;
const ENDPOINT_EPSILON: f32 = 0.025;
/// Material profiles are interpreted as nominal transmission through roughly one
/// interior-wall thickness. Bidirectional center probes scale that response from
/// actual scene geometry instead of treating a thin panel and a massive wall alike.
const ACOUSTIC_REFERENCE_THICKNESS_M: f32 = 0.18;
const MAX_RESOLVED_OCCLUDER_THICKNESS_M: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeDirection {
    ListenerToEmitter,
    EmitterToListenerCenter,
}

#[derive(Clone, Copy, Debug)]
struct PendingProbeRay {
    emitter_key: u64,
    listener_key: Option<u64>,
    sample_index: u8,
    max_t: f32,
    direction: ProbeDirection,
}

#[derive(Clone, Copy, Debug)]
struct AcousticCandidate {
    stable_key: u64,
    position: Vec3,
    distance: f32,
    settings: AudioOcclusionSettings,
}

#[derive(Clone, Debug)]
struct ProbeBlocker {
    entity_key: u64,
    distance: f32,
    max_t: f32,
    material_id: String,
    material: AcousticMaterialProfile,
}

#[derive(Clone, Debug, Default)]
struct ProbeAggregate {
    sample_count: u8,
    blocked: u8,
    center_blocked: bool,
    transmission_sum: f32,
    reflection_sum: f32,
    absorption_sum: f32,
    low_pass_sum: f32,
    materials: BTreeMap<String, u8>,
    center_forward: Option<ProbeBlocker>,
    center_reverse: Option<ProbeBlocker>,
}

/// Engine-owned physics-query contributor for spatial audio obstruction.
/// It emits provider-neutral `PhysicsQueryDto` batches only; Jolt/Bullet details
/// remain entirely behind the stable `engine.physics` transport.
pub struct AudioOcclusionPhysicsQueryProvider {
    pending: Mutex<BTreeMap<u64, PendingProbeRay>>,
    next_query: AtomicU64,
    fairness_cursor: AtomicU64,
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
            fairness_cursor: AtomicU64::new(0),
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
        self.select_candidate_budget(candidates)
    }

    fn select_candidate_budget(
        &self,
        candidates: Vec<AcousticCandidate>,
    ) -> Vec<AcousticCandidate> {
        let budget = self.max_emitters_per_tick.max(1);
        if candidates.len() <= budget {
            return candidates;
        }

        // Most probe capacity stays distance-prioritized because near sources dominate
        // perception and need fast edge response. The remaining quarter rotates through
        // the tail so crowded scenes cannot permanently starve occlusion for farther voices.
        let near_count = if budget <= 2 {
            1
        } else {
            (budget * 3 / 4).clamp(1, budget)
        };
        let near_count = near_count.min(candidates.len()).min(budget);
        let fair_slots = budget.saturating_sub(near_count);
        let mut selected = candidates[..near_count].to_vec();
        if fair_slots == 0 || near_count >= candidates.len() {
            return selected;
        }

        let tail = &candidates[near_count..];
        let start = (self
            .fairness_cursor
            .fetch_add(fair_slots.max(1) as u64, Ordering::Relaxed) as usize)
            % tail.len();
        for offset in 0..fair_slots.min(tail.len()) {
            selected.push(tail[(start + offset) % tail.len()]);
        }
        selected
    }

    fn build_probe_rays(
        &self,
        listener_state: newengine_audio_api::AudioListenerState,
        candidate: AcousticCandidate,
        listener_entity: Option<u64>,
        pending: &mut BTreeMap<u64, PendingProbeRay>,
    ) -> Vec<PhysicsQueryDto> {
        let listener_state = listener_state.sanitized();
        let listener = Vec3::new(
            listener_state.position[0],
            listener_state.position[1],
            listener_state.position[2],
        );
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
        let (left_ear, right_ear) = listener_state.ear_positions();
        let origins = [
            listener,
            Vec3::new(left_ear[0], left_ear[1], left_ear[2]),
            Vec3::new(right_ear[0], right_ear[1], right_ear[2]),
            listener,
            listener,
        ];
        let sample_count = candidate.settings.ray_count.clamp(1, 5);
        let mut queries = Vec::with_capacity(sample_count as usize);
        for sample_index in 0..sample_count {
            let origin = origins[sample_index as usize];
            let target = candidate.position + offsets[sample_index as usize];
            let ray = target - origin;
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
                    listener_key: listener_entity,
                    sample_index,
                    max_t,
                    direction: ProbeDirection::ListenerToEmitter,
                },
            );
            queries.push(PhysicsQueryDto {
                seq,
                ignore_entity: listener_entity,
                kind: PhysicsQueryKindDto::Ray {
                    origin: vec3_array(origin),
                    dir: vec3_array(dir),
                    max_t,
                },
            });

            // One reverse center ray gives us the far boundary of the dominant
            // occluder at very small query cost. If both rays hit the same entity,
            // `max_t - near - far` is a stable thickness estimate. Peripheral rays
            // stay one-way and continue to measure aperture/coverage.
            if sample_index == 0 {
                let reverse_seq = self.alloc_query_seq();
                pending.insert(
                    reverse_seq,
                    PendingProbeRay {
                        emitter_key: candidate.stable_key,
                        listener_key: listener_entity,
                        sample_index,
                        max_t,
                        direction: ProbeDirection::EmitterToListenerCenter,
                    },
                );
                queries.push(PhysicsQueryDto {
                    seq: reverse_seq,
                    ignore_entity: Some(candidate.stable_key),
                    kind: PhysicsQueryKindDto::Ray {
                        origin: vec3_array(target),
                        dir: vec3_array(-dir),
                        max_t,
                    },
                });
            }
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
            queries.extend(self.build_probe_rays(
                listener_state.listener,
                candidate,
                ignore_entity,
                &mut pending,
            ));
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
                    && probe.listener_key != Some(hit.entity)
                    && hit.distance.is_finite()
                    && hit.distance >= 0.0
                    && hit.distance + ENDPOINT_EPSILON < probe.max_t
            });
            let aggregate = aggregates.entry(probe.emitter_key).or_default();
            match probe.direction {
                ProbeDirection::ListenerToEmitter => {
                    aggregate.sample_count = aggregate.sample_count.saturating_add(1);
                    if let Some(hit) = blocker {
                        let blocker = resolve_probe_blocker(world, key_to_entity, hit, probe.max_t);
                        aggregate.blocked = aggregate.blocked.saturating_add(1);
                        if probe.sample_index == 0 {
                            aggregate.center_blocked = true;
                            aggregate.center_forward = Some(blocker.clone());
                        }
                        aggregate.transmission_sum += blocker.material.transmission_gain;
                        aggregate.reflection_sum += blocker.material.reflection_gain;
                        aggregate.absorption_sum += blocker.material.high_frequency_absorption;
                        aggregate.low_pass_sum += blocker.material.low_pass_hz;
                        *aggregate.materials.entry(blocker.material_id).or_insert(0) += 1;
                    }
                }
                ProbeDirection::EmitterToListenerCenter => {
                    if let Some(hit) = blocker {
                        aggregate.center_reverse = Some(resolve_probe_blocker(
                            world,
                            key_to_entity,
                            hit,
                            probe.max_t,
                        ));
                    }
                }
            }
        }

        for (emitter_key, aggregate) in aggregates {
            let samples = aggregate.sample_count.max(1);
            let blocked_samples = aggregate.blocked.min(samples);
            let obstruction = f32::from(blocked_samples) / f32::from(samples);
            let occlusion = occlusion_from_probe_coverage(obstruction, aggregate.center_blocked);
            let (estimated_thickness_m, center_blocker_layers) = center_path_geometry(&aggregate);
            let mut material = if blocked_samples > 0 {
                let count = f32::from(blocked_samples);
                AcousticMaterialProfile {
                    transmission_gain: aggregate.transmission_sum / count,
                    reflection_gain: aggregate.reflection_sum / count,
                    high_frequency_absorption: aggregate.absorption_sum / count,
                    low_pass_hz: aggregate.low_pass_sum / count,
                }
                .sanitized()
            } else {
                AcousticMaterialProfile {
                    transmission_gain: 1.0,
                    reflection_gain: 0.0,
                    high_frequency_absorption: 0.0,
                    low_pass_hz: 20_000.0,
                }
            };
            if estimated_thickness_m > ENDPOINT_EPSILON {
                material = material_response_for_thickness(material, estimated_thickness_m);
            } else if center_blocker_layers >= 2 {
                if let Some(reverse) = aggregate.center_reverse.as_ref() {
                    material = combine_material_layers(material, reverse.material);
                }
            }
            let dominant_blocker_entity = aggregate
                .center_forward
                .as_ref()
                .map(|blocker| blocker.entity_key);
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
                        estimated_thickness_m,
                        center_blocker_layers,
                        dominant_blocker_entity,
                        dominant_material,
                        material,
                    },
                );
            }
        }
        consumed
    }
}

pub(crate) fn resolve_acoustic_surface_for_entity(
    world: &World,
    blocker_entity: Option<EntityId>,
) -> AcousticSurface {
    if let Some(authored) = blocker_entity
        .and_then(|entity| world.get::<AcousticSurface>(entity))
        .cloned()
        .map(AcousticSurface::sanitized)
    {
        return authored;
    }
    let surface_id = blocker_entity
        .and_then(|entity| world.get::<PhysicsSurface>(entity))
        .map(|surface| surface.id.as_str())
        .unwrap_or("surface.default");
    world
        .resource::<AcousticMaterialLibrary>()
        .and_then(|library| library.resolve(surface_id))
        .unwrap_or_else(|| {
            // Geometry remains authoritative, but an unmapped physics surface must never inherit
            // an invented concrete/material response from engine code.
            AcousticSurface::new(surface_id, AcousticMaterialProfile::transparent())
        })
}

fn resolve_probe_blocker(
    world: &World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    hit: &PhysicsQueryHitDto,
    max_t: f32,
) -> ProbeBlocker {
    let surface =
        resolve_acoustic_surface_for_entity(world, key_to_entity.get(&hit.entity).copied());
    let material_id = surface.material_id;
    let material = surface.profile;
    ProbeBlocker {
        entity_key: hit.entity,
        distance: hit.distance.max(0.0),
        max_t,
        material_id,
        material,
    }
}

/// Derives center-path complexity from the two nearest boundary hits. Matching entities
/// describe opposite faces of one closed occluder and therefore yield an actual thickness.
/// Different entities prove at least two blocker layers even though the physics API remains
/// intentionally nearest-hit-only.
fn center_path_geometry(aggregate: &ProbeAggregate) -> (f32, u8) {
    let Some(forward) = aggregate.center_forward.as_ref() else {
        return (0.0, 0);
    };
    let Some(reverse) = aggregate.center_reverse.as_ref() else {
        return (0.0, 1);
    };
    if forward.entity_key != reverse.entity_key {
        return (0.0, 2);
    }
    let path_length = forward.max_t.min(reverse.max_t).max(0.0);
    let thickness = (path_length - forward.distance - reverse.distance)
        .clamp(0.0, MAX_RESOLVED_OCCLUDER_THICKNESS_M);
    (thickness, 1)
}

/// Scales an authored material response by measured geometric thickness. The material's
/// existing profile remains the authoring authority; geometry only changes how much of that
/// response accumulates along the direct path.
fn material_response_for_thickness(
    material: AcousticMaterialProfile,
    thickness_m: f32,
) -> AcousticMaterialProfile {
    let material = material.sanitized();
    let exponent = (thickness_m / ACOUSTIC_REFERENCE_THICKNESS_M).clamp(0.20, 6.0);
    let transmission_gain = material.transmission_gain.powf(exponent).clamp(0.0, 1.0);
    let high_frequency_gain = material
        .high_frequency_gain()
        .powf(exponent)
        .clamp(0.0, 1.0);
    let low_pass_hz = if exponent <= 1.0 {
        20_000.0 + (material.low_pass_hz - 20_000.0) * exponent
    } else {
        material.low_pass_hz * (-(exponent - 1.0) * 0.45).exp()
    };
    AcousticMaterialProfile {
        transmission_gain,
        // Thickness affects through-material propagation, not the authored boundary reflection.
        reflection_gain: material.reflection_gain,
        high_frequency_absorption: 1.0 - high_frequency_gain,
        low_pass_hz,
    }
    .sanitized()
}

fn combine_material_layers(
    a: AcousticMaterialProfile,
    b: AcousticMaterialProfile,
) -> AcousticMaterialProfile {
    let a = a.sanitized();
    let b = b.sanitized();
    AcousticMaterialProfile {
        transmission_gain: a.transmission_gain * b.transmission_gain,
        // A layered direct path keeps the first encountered boundary as its reflection authority.
        reflection_gain: a.reflection_gain,
        high_frequency_absorption: 1.0
            - (1.0 - a.high_frequency_absorption) * (1.0 - b.high_frequency_absorption),
        low_pass_hz: a.low_pass_hz.min(b.low_pass_hz),
    }
    .sanitized()
}

#[inline]
fn occlusion_from_probe_coverage(obstruction: f32, center_blocked: bool) -> f32 {
    let coverage = obstruction.clamp(0.0, 1.0);
    if coverage >= 0.999 {
        return 1.0;
    }
    // The direct center path carries more perceptual weight than peripheral aperture rays.
    // Peripheral blockage still contributes a small diffuse occlusion term instead of an
    // artificial binary zero, producing smooth transitions around door frames and wall edges.
    if center_blocked {
        (0.30 + coverage * 0.70).clamp(0.0, 1.0)
    } else {
        (coverage * coverage * 0.32).clamp(0.0, 1.0)
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
        assert_eq!(queries.len(), 8);
        assert!(queries
            .iter()
            .all(|query| query.seq & 0xfff0_0000_0000_0000 == AUDIO_OCCLUSION_QUERY_NAMESPACE));
    }

    #[test]
    fn crowded_scene_budget_keeps_nearest_emitter_and_rotates_fair_slots() {
        let mut world = listener_world();
        let mut emitter_keys = Vec::new();
        for index in 0..6 {
            let entity = world.spawn();
            emitter_keys.push(entity.stable_u64());
            let _ = world.insert(
                entity,
                Transform {
                    position: Vec3::new(0.0, 0.0, -(2.0 + index as f32 * 2.0)),
                    ..Transform::default()
                },
            );
            let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
            emitter.occlusion.ray_count = 1;
            let _ = world.insert(entity, emitter);
        }

        let provider = AudioOcclusionPhysicsQueryProvider::with_emitter_budget(2);
        let nearest = emitter_keys[0];
        let mut sampled = BTreeSet::new();
        for _ in 0..6 {
            let _ = provider.collect_queries(&world);
            let selected = provider
                .pending
                .lock()
                .values()
                .map(|probe| probe.emitter_key)
                .collect::<BTreeSet<_>>();
            assert!(selected.contains(&nearest));
            assert_eq!(selected.len(), 2);
            sampled.extend(selected);
        }
        assert_eq!(sampled.len(), emitter_keys.len());
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
        assert_eq!(queries.len(), 4);
        // sample 0 forward, sample 0 reverse, sample 1 forward, sample 2 forward
        let hit = PhysicsQueryHitDto {
            seq: queries[2].seq,
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
        assert_eq!(consumed.len(), 4);
        let observation = world
            .get::<AudioOcclusionObservation>(emitter_entity)
            .cloned()
            .expect("observation");
        assert_eq!(observation.samples, 3);
        assert_eq!(observation.blocked_samples, 1);
        assert!((observation.obstruction - 1.0 / 3.0).abs() < 1.0e-6);
        assert!(observation.occlusion > 0.0 && observation.occlusion < 0.2);
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
    fn bidirectional_center_probe_resolves_single_blocker_thickness() {
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
        emitter.occlusion.ray_count = 1;
        let _ = world.insert(emitter_entity, emitter);
        let blocker = world.spawn();
        let _ = world.insert(
            blocker,
            AcousticSurface::new(
                "material.test.wall",
                AcousticMaterialProfile {
                    transmission_gain: 0.40,
                    reflection_gain: 0.55,
                    high_frequency_absorption: 0.60,
                    low_pass_hz: 4_000.0,
                },
            ),
        );

        let provider = AudioOcclusionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        assert_eq!(queries.len(), 2);
        let hits = [
            PhysicsQueryHitDto {
                seq: queries[0].seq,
                entity: blocker.stable_u64(),
                position: [0.0, 0.0, -4.0],
                normal: [0.0, 0.0, 1.0],
                distance: 4.0,
            },
            PhysicsQueryHitDto {
                seq: queries[1].seq,
                entity: blocker.stable_u64(),
                position: [0.0, 0.0, -4.4],
                normal: [0.0, 0.0, -1.0],
                distance: 5.6,
            },
        ];
        let keys = world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect::<BTreeMap<_, _>>();
        provider.resolve_query_hits(&mut world, 11, &hits, &keys);
        let observation = world
            .get::<AudioOcclusionObservation>(emitter_entity)
            .expect("observation");
        assert!((observation.estimated_thickness_m - 0.4).abs() < 1.0e-4);
        assert_eq!(observation.center_blocker_layers, 1);
        assert!(observation.material.transmission_gain < 0.40);
        assert!(observation.material.low_pass_hz < 4_000.0);
    }

    #[test]
    fn thick_geometry_transmits_less_energy_and_high_frequency_than_thin_geometry() {
        let base = AcousticMaterialProfile {
            transmission_gain: 0.40,
            reflection_gain: 0.55,
            high_frequency_absorption: 0.60,
            low_pass_hz: 4_000.0,
        };
        let thin = material_response_for_thickness(base, 0.05);
        let reference = material_response_for_thickness(base, ACOUSTIC_REFERENCE_THICKNESS_M);
        let thick = material_response_for_thickness(base, 0.75);
        assert!(thin.transmission_gain > reference.transmission_gain);
        assert!(reference.transmission_gain > thick.transmission_gain);
        assert!(thin.high_frequency_absorption < reference.high_frequency_absorption);
        assert!(reference.high_frequency_absorption < thick.high_frequency_absorption);
        assert!(thin.low_pass_hz > reference.low_pass_hz);
        assert!(reference.low_pass_hz > thick.low_pass_hz);
    }

    #[test]
    fn distinct_center_blockers_accumulate_as_multiple_material_layers() {
        let a = AcousticMaterialProfile {
            transmission_gain: 0.5,
            reflection_gain: 0.45,
            high_frequency_absorption: 0.5,
            low_pass_hz: 5_000.0,
        };
        let b = AcousticMaterialProfile {
            transmission_gain: 0.4,
            reflection_gain: 0.35,
            high_frequency_absorption: 0.7,
            low_pass_hz: 3_000.0,
        };
        let combined = combine_material_layers(a, b);
        assert!((combined.transmission_gain - 0.2).abs() < 1.0e-6);
        assert!(combined.high_frequency_absorption > a.high_frequency_absorption);
        assert!(combined.high_frequency_absorption > b.high_frequency_absorption);
        assert_eq!(combined.low_pass_hz, 3_000.0);

        let aggregate = ProbeAggregate {
            center_forward: Some(ProbeBlocker {
                entity_key: 10,
                distance: 2.0,
                max_t: 10.0,
                material_id: "a".to_owned(),
                material: a,
            }),
            center_reverse: Some(ProbeBlocker {
                entity_key: 20,
                distance: 3.0,
                max_t: 10.0,
                material_id: "b".to_owned(),
                material: b,
            }),
            ..ProbeAggregate::default()
        };
        assert_eq!(center_path_geometry(&aggregate), (0.0, 2));
    }

    #[test]
    fn center_blockage_has_more_occlusion_weight_than_peripheral_blockage() {
        let peripheral = occlusion_from_probe_coverage(1.0 / 3.0, false);
        let center = occlusion_from_probe_coverage(1.0 / 3.0, true);
        assert!(center > peripheral * 4.0);
        assert!(center < 1.0);
    }

    #[test]
    fn authored_material_library_resolves_physics_surface_without_engine_presets() {
        let mut world = listener_world();
        world.insert_resource(AcousticMaterialLibrary::new(vec![
            newengine_audio_api::AcousticMaterialRule {
                material_id: "material.test.solid".to_owned(),
                surface_matches: vec!["test_solid".to_owned()],
                profile: AcousticMaterialProfile {
                    transmission_gain: 0.21,
                    reflection_gain: 0.67,
                    high_frequency_absorption: 0.81,
                    low_pass_hz: 2_100.0,
                },
            },
        ]));
        let blocker = world.spawn();
        let _ = world.insert(
            blocker,
            PhysicsSurface {
                id: "surface.wall.test_solid".to_owned(),
                ..PhysicsSurface::default()
            },
        );
        let keys = world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect::<BTreeMap<_, _>>();
        let hit = PhysicsQueryHitDto {
            seq: 1,
            entity: blocker.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: 1.0,
        };
        let resolved = resolve_probe_blocker(&world, &keys, &hit, 4.0);
        assert_eq!(resolved.material_id, "material.test.solid");
        assert!((resolved.material.transmission_gain - 0.21).abs() < 1.0e-6);
    }

    #[test]
    fn unmapped_physics_surface_uses_transparent_material_fallback() {
        let mut world = listener_world();
        let blocker = world.spawn();
        let _ = world.insert(
            blocker,
            PhysicsSurface {
                id: "surface.project.unknown".to_owned(),
                ..PhysicsSurface::default()
            },
        );
        let keys = world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect::<BTreeMap<_, _>>();
        let hit = PhysicsQueryHitDto {
            seq: 1,
            entity: blocker.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: 1.0,
        };
        let resolved = resolve_probe_blocker(&world, &keys, &hit, 4.0);
        assert_eq!(resolved.material_id, "surface.project.unknown");
        assert_eq!(resolved.material, AcousticMaterialProfile::transparent());
    }
}
