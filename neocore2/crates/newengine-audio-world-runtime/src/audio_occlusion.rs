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

use newengine_physics_world_api::{GameplayPhysicsQueryProvider, PhysicsSurface};

#[path = "audio_occlusion/acoustics.rs"]
mod acoustics;

pub(crate) use acoustics::resolve_acoustic_surface_for_entity;
use acoustics::{
    center_path_geometry, combine_material_layers, material_response_for_thickness,
    occlusion_from_probe_coverage, resolve_probe_blocker, AcousticCandidate, PendingProbeRay,
    ProbeAggregate, ProbeDirection,
};
#[cfg(test)]
use acoustics::{ProbeBlocker, ACOUSTIC_REFERENCE_THICKNESS_M};

const AUDIO_OCCLUSION_QUERY_NAMESPACE: u64 = 0xa0c0_0000_0000_0000;
const AUDIO_OCCLUSION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const DEFAULT_MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 32;
const MAX_OCCLUSION_EMITTERS_PER_TICK: usize = 256;
const ENDPOINT_EPSILON: f32 = 0.025;
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
        for (entity, emitter) in world.query::<AudioEmitter>() {
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

        let ignore_entity = listener_state.listener_entity;
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
    newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_AUDIO_OCCLUSION_MAX_EMITTERS_PER_TICK")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_OCCLUSION_EMITTERS_PER_TICK))
        .unwrap_or(DEFAULT_MAX_OCCLUSION_EMITTERS_PER_TICK)
}

#[cfg(test)]
#[path = "audio_occlusion/tests.rs"]
mod tests;
