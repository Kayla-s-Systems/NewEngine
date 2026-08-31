use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use newengine_audio_api::{AcousticMaterialProfile, AudioEmitter, AudioEnvironmentZone};
use newengine_audio_world_api::{
    first_order_reflection_geometry, second_order_reflection_geometry,
    AudioEarlyReflectionObservation, AudioEarlyReflectionPathObservation,
    AudioFirstOrderReflectionGeometry, AudioListenerRuntimeState, AudioRoomObbGeometry,
    AudioSecondOrderReflectionGeometry, AudioSecondOrderReflectionPathObservation,
};
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_transform::{read_entity_world_pose_local_chain, GlobalTransform, Transform};
use parking_lot::Mutex;

use crate::audio_occlusion::resolve_acoustic_surface_for_entity;
use crate::gameplay::{first_player, GameplayPhysicsQueryProvider};

const AUDIO_REFLECTION_QUERY_NAMESPACE: u64 = 0xa0d0_0000_0000_0000;
const AUDIO_REFLECTION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const MAX_REFLECTION_EMITTERS_PER_TICK: usize = 12;
const MAX_SECOND_ORDER_PATHS_PER_EMITTER: usize = 4;
const REFLECTION_ENDPOINT_EPSILON: f32 = 0.04;
const SECOND_ORDER_MIDDLE_SEGMENT_EPSILON: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReflectionProbeLeg {
    Source,
    Listener,
}

#[derive(Clone, Copy, Debug)]
struct PendingReflectionRay {
    emitter_key: u64,
    leg: ReflectionProbeLeg,
    geometry: AudioFirstOrderReflectionGeometry,
    max_t: f32,
    source_position: [f32; 3],
    listener_position: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
struct LegResolution {
    blocked: bool,
    endpoint_entity: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct ReflectionAggregate {
    geometry: AudioFirstOrderReflectionGeometry,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    source: LegResolution,
    listener: LegResolution,
}

impl ReflectionAggregate {
    fn new(ray: PendingReflectionRay) -> Self {
        Self {
            geometry: ray.geometry,
            source_position: ray.source_position,
            listener_position: ray.listener_position,
            source: LegResolution::default(),
            listener: LegResolution::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecondOrderProbeLeg {
    Source,
    Middle,
    Listener,
}

#[derive(Clone, Copy, Debug)]
struct PendingSecondOrderRay {
    emitter_key: u64,
    leg: SecondOrderProbeLeg,
    geometry: AudioSecondOrderReflectionGeometry,
    max_t: f32,
    source_position: [f32; 3],
    listener_position: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
struct SecondOrderAggregate {
    geometry: AudioSecondOrderReflectionGeometry,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    source: LegResolution,
    middle_blocked: bool,
    listener: LegResolution,
}

impl SecondOrderAggregate {
    fn new(ray: PendingSecondOrderRay) -> Self {
        Self {
            geometry: ray.geometry,
            source_position: ray.source_position,
            listener_position: ray.listener_position,
            source: LegResolution::default(),
            middle_blocked: false,
            listener: LegResolution::default(),
        }
    }
}

#[derive(Clone, Copy)]
struct ReflectionEmitterCandidate {
    key: u64,
    position: Vec3,
    distance: f32,
}

/// Bounded first-order reflection visibility contributor. It emits only provider-neutral
/// `PhysicsQueryDto::Ray` segments; room/material semantics remain engine/audio-domain data.
pub struct AudioReflectionPhysicsQueryProvider {
    pending: Mutex<BTreeMap<u64, PendingReflectionRay>>,
    pending_second_order: Mutex<BTreeMap<u64, PendingSecondOrderRay>>,
    next_query: AtomicU64,
}

impl Default for AudioReflectionPhysicsQueryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioReflectionPhysicsQueryProvider {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(BTreeMap::new()),
            pending_second_order: Mutex::new(BTreeMap::new()),
            next_query: AtomicU64::new(1),
        }
    }

    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    fn alloc_query_id(&self) -> u64 {
        AUDIO_REFLECTION_QUERY_NAMESPACE
            | (self.next_query.fetch_add(1, Ordering::Relaxed)
                & AUDIO_REFLECTION_QUERY_COUNTER_MASK)
    }

    fn emitter_candidates(&self, world: &World, listener: Vec3) -> Vec<ReflectionEmitterCandidate> {
        let mut candidates = world
            .query::<AudioEmitter>()
            .filter_map(|(entity, emitter)| {
                if !emitter.enabled || !emitter.spatial {
                    return None;
                }
                let position = entity_world_position(world, entity)?;
                let distance = position.distance(listener);
                distance.is_finite().then_some(ReflectionEmitterCandidate {
                    key: entity.stable_u64(),
                    position,
                    distance,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| a.key.cmp(&b.key))
        });
        candidates.truncate(MAX_REFLECTION_EMITTERS_PER_TICK);
        candidates
    }

    fn room_geometry(
        &self,
        world: &World,
        source: Vec3,
        listener: Vec3,
    ) -> Option<AudioRoomObbGeometry> {
        let source_array = [source.x, source.y, source.z];
        let listener_array = [listener.x, listener.y, listener.z];
        let mut rooms = world
            .query::<AudioEnvironmentZone>()
            .filter_map(|(entity, zone)| {
                let zone = zone.clone().sanitized();
                if !zone.enabled {
                    return None;
                }
                let (center, rotation) = entity_world_pose(world, entity)?;
                let scale = world_scale(world, entity);
                let geometry = AudioRoomObbGeometry {
                    center: [center.x, center.y, center.z],
                    rotation_xyzw: [rotation.x, rotation.y, rotation.z, rotation.w],
                    half_extents: [
                        zone.half_extents[0] * scale.x.abs().max(1.0e-4),
                        zone.half_extents[1] * scale.y.abs().max(1.0e-4),
                        zone.half_extents[2] * scale.z.abs().max(1.0e-4),
                    ],
                };
                (!first_order_reflection_geometry(geometry, source_array, listener_array)
                    .is_empty())
                .then_some((zone.priority, entity.stable_u64(), geometry))
            })
            .collect::<Vec<_>>();
        rooms.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        rooms.into_iter().next().map(|(_, _, geometry)| geometry)
    }

    fn push_leg(
        &self,
        pending: &mut BTreeMap<u64, PendingReflectionRay>,
        queries: &mut Vec<PhysicsQueryDto>,
        emitter: ReflectionEmitterCandidate,
        listener: Vec3,
        geometry: AudioFirstOrderReflectionGeometry,
        leg: ReflectionProbeLeg,
        listener_entity: Option<u64>,
    ) {
        let point = Vec3::new(
            geometry.reflection_point[0],
            geometry.reflection_point[1],
            geometry.reflection_point[2],
        );
        let origin = match leg {
            ReflectionProbeLeg::Source => emitter.position,
            ReflectionProbeLeg::Listener => listener,
        };
        let delta = point - origin;
        let max_t = delta.length();
        if !max_t.is_finite() || max_t <= 1.0e-4 {
            return;
        }
        let dir = delta / max_t;
        let seq = self.alloc_query_id();
        pending.insert(
            seq,
            PendingReflectionRay {
                emitter_key: emitter.key,
                leg,
                geometry,
                max_t,
                source_position: [emitter.position.x, emitter.position.y, emitter.position.z],
                listener_position: [listener.x, listener.y, listener.z],
            },
        );
        queries.push(PhysicsQueryDto {
            seq,
            ignore_entity: match leg {
                ReflectionProbeLeg::Source => Some(emitter.key),
                ReflectionProbeLeg::Listener => listener_entity,
            },
            kind: PhysicsQueryKindDto::Ray {
                origin: [origin.x, origin.y, origin.z],
                dir: [dir.x, dir.y, dir.z],
                max_t,
            },
        });
    }

    fn push_second_order_leg(
        &self,
        pending: &mut BTreeMap<u64, PendingSecondOrderRay>,
        queries: &mut Vec<PhysicsQueryDto>,
        emitter: ReflectionEmitterCandidate,
        listener: Vec3,
        geometry: AudioSecondOrderReflectionGeometry,
        leg: SecondOrderProbeLeg,
        listener_entity: Option<u64>,
    ) {
        let first_point = geometry.reflection_points[0];
        let second_point = geometry.reflection_points[1];
        let first = Vec3::new(first_point[0], first_point[1], first_point[2]);
        let second = Vec3::new(second_point[0], second_point[1], second_point[2]);
        let (mut origin, mut target, ignore_entity) = match leg {
            SecondOrderProbeLeg::Source => (emitter.position, first, Some(emitter.key)),
            SecondOrderProbeLeg::Middle => (first, second, None),
            SecondOrderProbeLeg::Listener => (listener, second, listener_entity),
        };
        if leg == SecondOrderProbeLeg::Middle {
            let segment = target - origin;
            let length = segment.length();
            if !length.is_finite() || length <= SECOND_ORDER_MIDDLE_SEGMENT_EPSILON * 2.0 {
                return;
            }
            let direction = segment / length;
            origin += direction * SECOND_ORDER_MIDDLE_SEGMENT_EPSILON;
            target -= direction * SECOND_ORDER_MIDDLE_SEGMENT_EPSILON;
        }
        let delta = target - origin;
        let max_t = delta.length();
        if !max_t.is_finite() || max_t <= 1.0e-4 {
            return;
        }
        let dir = delta / max_t;
        let seq = self.alloc_query_id();
        pending.insert(
            seq,
            PendingSecondOrderRay {
                emitter_key: emitter.key,
                leg,
                geometry,
                max_t,
                source_position: [emitter.position.x, emitter.position.y, emitter.position.z],
                listener_position: [listener.x, listener.y, listener.z],
            },
        );
        queries.push(PhysicsQueryDto {
            seq,
            ignore_entity,
            kind: PhysicsQueryKindDto::Ray {
                origin: [origin.x, origin.y, origin.z],
                dir: [dir.x, dir.y, dir.z],
                max_t,
            },
        });
    }
}

impl GameplayPhysicsQueryProvider for AudioReflectionPhysicsQueryProvider {
    fn id(&self) -> &'static str {
        "engine.audio.physics-reflections"
    }

    fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto> {
        let Some(listener_state) = world.resource::<AudioListenerRuntimeState>().copied() else {
            self.pending.lock().clear();
            self.pending_second_order.lock().clear();
            return Vec::new();
        };
        let listener_array = listener_state.listener.sanitized().position;
        let listener = Vec3::new(listener_array[0], listener_array[1], listener_array[2]);
        if !listener.is_finite() {
            self.pending.lock().clear();
            self.pending_second_order.lock().clear();
            return Vec::new();
        }
        let listener_entity = first_player(world).map(EntityId::stable_u64);
        let mut pending = BTreeMap::new();
        let mut pending_second_order = BTreeMap::new();
        let mut queries = Vec::new();
        for emitter in self.emitter_candidates(world, listener) {
            let Some(room) = self.room_geometry(world, emitter.position, listener) else {
                continue;
            };
            let source = [emitter.position.x, emitter.position.y, emitter.position.z];
            let receiver = [listener.x, listener.y, listener.z];
            for geometry in first_order_reflection_geometry(room, source, receiver) {
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    emitter,
                    listener,
                    geometry,
                    ReflectionProbeLeg::Source,
                    listener_entity,
                );
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    emitter,
                    listener,
                    geometry,
                    ReflectionProbeLeg::Listener,
                    listener_entity,
                );
            }
            let mut second_order = second_order_reflection_geometry(room, source, receiver);
            second_order.truncate(MAX_SECOND_ORDER_PATHS_PER_EMITTER);
            for geometry in second_order {
                for leg in [
                    SecondOrderProbeLeg::Source,
                    SecondOrderProbeLeg::Middle,
                    SecondOrderProbeLeg::Listener,
                ] {
                    self.push_second_order_leg(
                        &mut pending_second_order,
                        &mut queries,
                        emitter,
                        listener,
                        geometry,
                        leg,
                        listener_entity,
                    );
                }
            }
        }
        *self.pending.lock() = pending;
        *self.pending_second_order.lock() = pending_second_order;
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
        let pending_second_order = std::mem::take(&mut *self.pending_second_order.lock());
        if pending.is_empty() && pending_second_order.is_empty() {
            return BTreeSet::new();
        }
        let hits_by_seq = hits
            .iter()
            .map(|hit| (hit.seq, *hit))
            .collect::<BTreeMap<_, _>>();
        let mut consumed = BTreeSet::new();
        let mut aggregates = BTreeMap::<(u64, u8), ReflectionAggregate>::new();
        for (seq, ray) in pending {
            consumed.insert(seq);
            let resolution = if let Some(hit) = hits_by_seq.get(&seq).filter(|hit| {
                hit.entity != ray.emitter_key && hit.distance.is_finite() && hit.distance >= 0.0
            }) {
                if hit.distance + REFLECTION_ENDPOINT_EPSILON < ray.max_t {
                    LegResolution {
                        blocked: true,
                        endpoint_entity: None,
                    }
                } else {
                    LegResolution {
                        blocked: false,
                        endpoint_entity: Some(hit.entity),
                    }
                }
            } else {
                LegResolution::default()
            };
            let aggregate = aggregates
                .entry((ray.emitter_key, ray.geometry.face_index))
                .or_insert_with(|| ReflectionAggregate::new(ray));
            match ray.leg {
                ReflectionProbeLeg::Source => aggregate.source = resolution,
                ReflectionProbeLeg::Listener => aggregate.listener = resolution,
            }
        }

        let mut second_aggregates = BTreeMap::<(u64, [u8; 2]), SecondOrderAggregate>::new();
        for (seq, ray) in pending_second_order {
            consumed.insert(seq);
            let hit = hits_by_seq.get(&seq).filter(|hit| {
                hit.entity != ray.emitter_key && hit.distance.is_finite() && hit.distance >= 0.0
            });
            let aggregate = second_aggregates
                .entry((ray.emitter_key, ray.geometry.face_indices))
                .or_insert_with(|| SecondOrderAggregate::new(ray));
            match ray.leg {
                SecondOrderProbeLeg::Source | SecondOrderProbeLeg::Listener => {
                    let resolution = if let Some(hit) = hit {
                        if hit.distance + REFLECTION_ENDPOINT_EPSILON < ray.max_t {
                            LegResolution {
                                blocked: true,
                                endpoint_entity: None,
                            }
                        } else {
                            LegResolution {
                                blocked: false,
                                endpoint_entity: Some(hit.entity),
                            }
                        }
                    } else {
                        LegResolution::default()
                    };
                    match ray.leg {
                        SecondOrderProbeLeg::Source => aggregate.source = resolution,
                        SecondOrderProbeLeg::Listener => aggregate.listener = resolution,
                        SecondOrderProbeLeg::Middle => unreachable!(),
                    }
                }
                SecondOrderProbeLeg::Middle => {
                    aggregate.middle_blocked =
                        hit.is_some_and(|hit| hit.distance <= ray.max_t + 1.0e-4);
                }
            }
        }

        let mut by_emitter = BTreeMap::<u64, Vec<AudioEarlyReflectionPathObservation>>::new();
        let mut second_by_emitter =
            BTreeMap::<u64, Vec<AudioSecondOrderReflectionPathObservation>>::new();
        let mut positions = BTreeMap::<u64, ([f32; 3], [f32; 3])>::new();
        for ((emitter_key, _), aggregate) in aggregates {
            let visible = !aggregate.source.blocked && !aggregate.listener.blocked;
            let boundary_entity = match (
                aggregate.source.endpoint_entity,
                aggregate.listener.endpoint_entity,
            ) {
                (Some(a), Some(b)) if a == b => Some(a),
                (Some(a), None) | (None, Some(a)) => Some(a),
                _ => None,
            };
            let material_known = boundary_entity.is_some();
            let material = boundary_entity
                .and_then(|key| key_to_entity.get(&key).copied())
                .map(|entity| resolve_acoustic_surface_for_entity(world, Some(entity)).profile)
                .unwrap_or_else(AcousticMaterialProfile::transparent);
            positions.insert(
                emitter_key,
                (aggregate.source_position, aggregate.listener_position),
            );
            by_emitter
                .entry(emitter_key)
                .or_default()
                .push(AudioEarlyReflectionPathObservation {
                    face_index: aggregate.geometry.face_index,
                    visible,
                    boundary_entity,
                    reflection_point: aggregate.geometry.reflection_point,
                    arrival_direction: aggregate.geometry.arrival_direction,
                    path_length_m: aggregate.geometry.path_length_m,
                    excess_length_m: aggregate.geometry.excess_length_m,
                    material_known,
                    material,
                });
        }

        for ((emitter_key, _), aggregate) in second_aggregates {
            let visible = !aggregate.source.blocked
                && !aggregate.middle_blocked
                && !aggregate.listener.blocked;
            let boundary_entities = [
                aggregate.source.endpoint_entity,
                aggregate.listener.endpoint_entity,
            ];
            let material_known = boundary_entities.map(|entity| entity.is_some());
            let materials = boundary_entities.map(|boundary| {
                boundary
                    .and_then(|key| key_to_entity.get(&key).copied())
                    .map(|entity| resolve_acoustic_surface_for_entity(world, Some(entity)).profile)
                    .unwrap_or_else(AcousticMaterialProfile::transparent)
            });
            positions.insert(
                emitter_key,
                (aggregate.source_position, aggregate.listener_position),
            );
            second_by_emitter.entry(emitter_key).or_default().push(
                AudioSecondOrderReflectionPathObservation {
                    face_indices: aggregate.geometry.face_indices,
                    visible,
                    boundary_entities,
                    reflection_points: aggregate.geometry.reflection_points,
                    arrival_direction: aggregate.geometry.arrival_direction,
                    path_length_m: aggregate.geometry.path_length_m,
                    excess_length_m: aggregate.geometry.excess_length_m,
                    material_known,
                    materials,
                },
            );
        }

        let emitter_keys = by_emitter
            .keys()
            .chain(second_by_emitter.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        for emitter_key in emitter_keys {
            let Some(entity) = key_to_entity.get(&emitter_key).copied() else {
                continue;
            };
            let mut paths = by_emitter.remove(&emitter_key).unwrap_or_default();
            paths.sort_by(|a, b| {
                a.path_length_m
                    .total_cmp(&b.path_length_m)
                    .then_with(|| a.face_index.cmp(&b.face_index))
            });
            let mut second_order_paths = second_by_emitter.remove(&emitter_key).unwrap_or_default();
            second_order_paths.sort_by(|a, b| {
                a.path_length_m
                    .total_cmp(&b.path_length_m)
                    .then_with(|| a.face_indices.cmp(&b.face_indices))
            });
            let (source_position, listener_position) = positions
                .get(&emitter_key)
                .copied()
                .unwrap_or(([0.0; 3], [0.0; 3]));
            let _ = world.insert(
                entity,
                AudioEarlyReflectionObservation {
                    fixed_tick,
                    source_position,
                    listener_position,
                    paths,
                    second_order_paths,
                },
            );
        }
        consumed
    }
}

fn entity_world_position(world: &World, entity: EntityId) -> Option<Vec3> {
    entity_world_pose(world, entity).map(|(position, _)| position)
}

fn entity_world_pose(world: &World, entity: EntityId) -> Option<(Vec3, newengine_math::Quat)> {
    read_entity_world_pose_local_chain(world, entity).or_else(|| {
        world.get::<Transform>(entity).map(|transform| {
            (
                transform.position,
                transform.rotation.normalize_or_identity(),
            )
        })
    })
}

fn world_scale(world: &World, entity: EntityId) -> Vec3 {
    if let Some(global) = world.get::<GlobalTransform>(entity) {
        let (scale, _, _) = global.0.to_scale_rotation_translation();
        if scale.x.is_finite() && scale.y.is_finite() && scale.z.is_finite() {
            return scale;
        }
    }
    world
        .get::<Transform>(entity)
        .map(|transform| transform.scale)
        .unwrap_or(Vec3::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::{AcousticMaterialProfile, AcousticSurface, AudioListenerState};

    fn reflection_world() -> (World, EntityId) {
        let mut world = World::new();
        world.insert_resource(AudioListenerRuntimeState {
            listener: AudioListenerState::default(),
            frame_index: 1,
        });

        let room = world.spawn();
        let _ = world.insert(room, Transform::default());
        let _ = world.insert(
            room,
            AudioEnvironmentZone {
                zone_id: "room.reflection-test".to_owned(),
                half_extents: [5.0, 4.0, 6.0],
                blend_distance: 0.0,
                ..AudioEnvironmentZone::default()
            },
        );

        let emitter = world.spawn();
        let _ = world.insert(
            emitter,
            Transform {
                position: Vec3::new(1.0, 0.5, -1.0),
                ..Transform::default()
            },
        );
        let _ = world.insert(
            emitter,
            AudioEmitter::new("shared/audio/test.yscd@reflection"),
        );
        (world, emitter)
    }

    fn entity_keys(world: &World) -> BTreeMap<u64, EntityId> {
        world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect()
    }

    fn ray_max_t(query: &PhysicsQueryDto) -> f32 {
        match query.kind {
            PhysicsQueryKindDto::Ray { max_t, .. } => max_t,
            _ => panic!("reflection contributor must emit ray queries"),
        }
    }

    #[test]
    fn provider_emits_two_visibility_legs_for_each_first_order_room_face() {
        let (world, _) = reflection_world();
        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        assert_eq!(provider.pending.lock().len(), 12);
        assert_eq!(provider.pending_second_order.lock().len(), 12);
        assert_eq!(queries.len(), 24);
        assert!(queries.iter().all(|query| {
            query.seq & 0xfff0_0000_0000_0000 == AUDIO_REFLECTION_QUERY_NAMESPACE
        }));
    }

    #[test]
    fn clear_reflection_probes_publish_visible_material_unknown_paths() {
        let (mut world, emitter) = reflection_world();
        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let keys = entity_keys(&world);
        let consumed = provider.resolve_query_hits(&mut world, 7, &[], &keys);
        assert_eq!(consumed.len(), queries.len());
        let observation = world
            .get::<AudioEarlyReflectionObservation>(emitter)
            .expect("reflection observation");
        assert_eq!(observation.paths.len(), 6);
        assert_eq!(observation.second_order_paths.len(), 4);
        assert!(observation.paths.iter().all(|path| path.visible));
        assert!(observation.paths.iter().all(|path| !path.material_known));
        assert!(observation
            .second_order_paths
            .iter()
            .all(|path| path.visible));
        assert!(observation
            .second_order_paths
            .iter()
            .all(|path| path.material_known == [false; 2]));
    }

    #[test]
    fn blocker_before_reflection_point_closes_only_that_first_order_path() {
        let (mut world, emitter) = reflection_world();
        let blocker = world.spawn();
        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let query = &queries[0];
        let max_t = ray_max_t(query);
        assert!(max_t > 0.5);
        let hit = PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: query.seq,
            entity: blocker.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: max_t * 0.5,
        };
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 8, &[hit], &keys);
        let observation = world
            .get::<AudioEarlyReflectionObservation>(emitter)
            .expect("reflection observation");
        assert_eq!(
            observation
                .paths
                .iter()
                .filter(|path| !path.visible)
                .count(),
            1
        );
        assert_eq!(
            observation.paths.iter().filter(|path| path.visible).count(),
            5
        );
    }

    #[test]
    fn endpoint_hit_resolves_authored_boundary_reflection_material() {
        let (mut world, emitter) = reflection_world();
        let boundary = world.spawn();
        let authored = AcousticMaterialProfile {
            transmission_gain: 0.18,
            reflection_gain: 0.91,
            high_frequency_absorption: 0.24,
            low_pass_hz: 7_500.0,
        };
        let _ = world.insert(
            boundary,
            AcousticSurface::new("material.test.reflective", authored),
        );

        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let query = &queries[0];
        let max_t = ray_max_t(query);
        let hit = PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: query.seq,
            entity: boundary.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: max_t,
        };
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 9, &[hit], &keys);
        let observation = world
            .get::<AudioEarlyReflectionObservation>(emitter)
            .expect("reflection observation");
        let resolved = observation
            .paths
            .iter()
            .find(|path| path.material_known)
            .expect("authored endpoint material");
        assert!(resolved.visible);
        assert_eq!(resolved.boundary_entity, Some(boundary.stable_u64()));
        assert!((resolved.material.reflection_gain - 0.91).abs() < 1.0e-6);
        assert!((resolved.material.high_frequency_absorption - 0.24).abs() < 1.0e-6);
    }

    #[test]
    fn second_order_middle_blocker_closes_only_its_three_leg_path() {
        let (mut world, emitter) = reflection_world();
        let blocker = world.spawn();
        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let (seq, pending) = provider
            .pending_second_order
            .lock()
            .iter()
            .find(|(_, pending)| pending.leg == SecondOrderProbeLeg::Middle)
            .map(|(seq, pending)| (*seq, *pending))
            .expect("second-order middle leg");
        let query = queries
            .iter()
            .find(|query| query.seq == seq)
            .expect("middle query");
        let max_t = ray_max_t(query);
        let hit = PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,
            seq,
            entity: blocker.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: max_t * 0.5,
        };
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 10, &[hit], &keys);
        let observation = world
            .get::<AudioEarlyReflectionObservation>(emitter)
            .expect("reflection observation");
        let blocked = observation
            .second_order_paths
            .iter()
            .filter(|path| !path.visible)
            .collect::<Vec<_>>();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].face_indices, pending.geometry.face_indices);
        assert!(observation.paths.iter().all(|path| path.visible));
    }

    #[test]
    fn second_order_endpoint_hits_resolve_two_independent_boundary_materials() {
        let (mut world, emitter) = reflection_world();
        let first_boundary = world.spawn();
        let second_boundary = world.spawn();
        let first_material = AcousticMaterialProfile {
            transmission_gain: 0.2,
            reflection_gain: 0.82,
            high_frequency_absorption: 0.25,
            low_pass_hz: 7_000.0,
        };
        let second_material = AcousticMaterialProfile {
            transmission_gain: 0.3,
            reflection_gain: 0.55,
            high_frequency_absorption: 0.60,
            low_pass_hz: 3_500.0,
        };
        let _ = world.insert(
            first_boundary,
            AcousticSurface::new("material.test.first", first_material),
        );
        let _ = world.insert(
            second_boundary,
            AcousticSurface::new("material.test.second", second_material),
        );

        let provider = AudioReflectionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let pending = provider.pending_second_order.lock();
        let target_faces = pending
            .values()
            .find(|pending| pending.leg == SecondOrderProbeLeg::Source)
            .map(|pending| pending.geometry.face_indices)
            .expect("second-order path");
        let source = pending
            .iter()
            .find(|(_, pending)| {
                pending.geometry.face_indices == target_faces
                    && pending.leg == SecondOrderProbeLeg::Source
            })
            .map(|(seq, pending)| (*seq, *pending))
            .expect("source endpoint leg");
        let listener = pending
            .iter()
            .find(|(_, pending)| {
                pending.geometry.face_indices == target_faces
                    && pending.leg == SecondOrderProbeLeg::Listener
            })
            .map(|(seq, pending)| (*seq, *pending))
            .expect("listener endpoint leg");
        drop(pending);
        let source_max = ray_max_t(
            queries
                .iter()
                .find(|query| query.seq == source.0)
                .expect("source query"),
        );
        let listener_max = ray_max_t(
            queries
                .iter()
                .find(|query| query.seq == listener.0)
                .expect("listener query"),
        );
        let hits = [
            PhysicsQueryHitDto {
                subshape_id: 0,
                hit_index: 0,
                back_face: false,

                seq: source.0,
                entity: first_boundary.stable_u64(),
                position: source.1.geometry.reflection_points[0],
                normal: [0.0, 1.0, 0.0],
                distance: source_max,
            },
            PhysicsQueryHitDto {
                subshape_id: 0,
                hit_index: 0,
                back_face: false,

                seq: listener.0,
                entity: second_boundary.stable_u64(),
                position: listener.1.geometry.reflection_points[1],
                normal: [0.0, 1.0, 0.0],
                distance: listener_max,
            },
        ];
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 11, &hits, &keys);
        let observation = world
            .get::<AudioEarlyReflectionObservation>(emitter)
            .expect("reflection observation");
        let path = observation
            .second_order_paths
            .iter()
            .find(|path| path.face_indices == target_faces)
            .expect("resolved second-order path");
        assert!(path.visible);
        assert_eq!(
            path.boundary_entities,
            [
                Some(first_boundary.stable_u64()),
                Some(second_boundary.stable_u64()),
            ]
        );
        assert_eq!(path.material_known, [true, true]);
        assert_eq!(path.materials, [first_material, second_material]);
    }
}
