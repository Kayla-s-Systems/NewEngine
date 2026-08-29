#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use newengine_audio_api::{
    AcousticMaterialLibrary, AcousticMaterialProfile, AcousticSurface, AudioEmitter,
};
use newengine_audio_world_api::{
    edge_diffraction_geometry, mesh_diffraction_edges, AudioEdgeDiffractionGeometry,
    AudioEdgeDiffractionObservation, AudioEdgeDiffractionPathObservation,
    AudioListenerRuntimeState, AudioOcclusionObservation,
};
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_transform::{read_entity_world_pose_local_chain, Transform};
use parking_lot::Mutex;

use crate::audio_occlusion::resolve_acoustic_surface_for_entity;
use crate::gameplay::{
    first_player, GameplayPhysicsQueryProvider, PhysicsSurface, StaticMeshCollider,
};

const AUDIO_DIFFRACTION_QUERY_NAMESPACE: u64 = 0xa0d0_0000_0000_0000;
const AUDIO_DIFFRACTION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const MAX_DIFFRACTION_EMITTERS_PER_TICK: usize = 8;
const MAX_EDGE_CANDIDATES_PER_EMITTER: usize = 6;
const EDGE_VISIBILITY_ENDPOINT_EPSILON: f32 = 0.04;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffractionProbeLeg {
    Source,
    Listener,
}

#[derive(Clone, Copy, Debug)]
struct CachedWorldEdge {
    vertex_indices: [u32; 2],
    endpoints: [[f32; 3]; 2],
    wedge_angle_radians: f32,
}

#[derive(Clone, Debug)]
struct CachedBlockerEdges {
    revision: u64,
    edges: Arc<[CachedWorldEdge]>,
}

#[derive(Clone, Copy, Debug)]
struct DiffractionEmitterCandidate {
    emitter_key: u64,
    position: Vec3,
    distance: f32,
    blocker_key: u64,
    blocker_entity: EntityId,
}

#[derive(Clone, Copy, Debug)]
struct PendingDiffractionRay {
    emitter_key: u64,
    listener_key: Option<u64>,
    blocker_key: u64,
    leg: DiffractionProbeLeg,
    edge: CachedWorldEdge,
    geometry: AudioEdgeDiffractionGeometry,
    max_t: f32,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    material_known: bool,
    material: AcousticMaterialProfile,
}

#[derive(Clone, Copy, Debug)]
struct DiffractionAggregate {
    edge: CachedWorldEdge,
    geometry: AudioEdgeDiffractionGeometry,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    source_blocked: bool,
    listener_blocked: bool,
    material_known: bool,
    material: AcousticMaterialProfile,
}

impl DiffractionAggregate {
    fn new(ray: PendingDiffractionRay) -> Self {
        Self {
            edge: ray.edge,
            geometry: ray.geometry,
            source_position: ray.source_position,
            listener_position: ray.listener_position,
            source_blocked: false,
            listener_blocked: false,
            material_known: ray.material_known,
            material: ray.material,
        }
    }
}

/// Bounded edge-diffraction query contributor. Candidate edges come only from the canonical static
/// mesh collider that the previous direct-path occlusion observation identified as the blocker.
pub struct AudioDiffractionPhysicsQueryProvider {
    edge_cache: Mutex<BTreeMap<u64, CachedBlockerEdges>>,
    pending: Mutex<BTreeMap<u64, PendingDiffractionRay>>,
    tracked_emitters: Mutex<BTreeSet<u64>>,
    clear_emitters: Mutex<BTreeSet<u64>>,
    next_query: AtomicU64,
}

impl Default for AudioDiffractionPhysicsQueryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDiffractionPhysicsQueryProvider {
    pub fn new() -> Self {
        Self {
            edge_cache: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(BTreeMap::new()),
            tracked_emitters: Mutex::new(BTreeSet::new()),
            clear_emitters: Mutex::new(BTreeSet::new()),
            next_query: AtomicU64::new(1),
        }
    }

    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    #[inline]
    fn alloc_query_id(&self) -> u64 {
        let value =
            self.next_query.fetch_add(1, Ordering::Relaxed) & AUDIO_DIFFRACTION_QUERY_COUNTER_MASK;
        AUDIO_DIFFRACTION_QUERY_NAMESPACE | value.max(1)
    }

    fn static_mesh_entities(world: &World) -> BTreeMap<u64, EntityId> {
        world
            .query::<StaticMeshCollider>()
            .map(|(entity, _)| (entity.stable_u64(), entity))
            .collect()
    }

    fn collect_candidates(
        &self,
        world: &World,
        listener: Vec3,
        mesh_entities: &BTreeMap<u64, EntityId>,
    ) -> Vec<DiffractionEmitterCandidate> {
        let mut candidates = Vec::new();
        for entity in world.iter_entities() {
            let Some(emitter) = world.get::<AudioEmitter>(entity) else {
                continue;
            };
            let Some(observation) = world.get::<AudioOcclusionObservation>(entity) else {
                continue;
            };
            if !emitter.enabled
                || !emitter.spatial
                || emitter.cue.trim().is_empty()
                || observation.occlusion <= 1.0e-4
            {
                continue;
            }
            let Some(blocker_key) = observation.dominant_blocker_entity else {
                continue;
            };
            let Some(blocker_entity) = mesh_entities.get(&blocker_key).copied() else {
                continue;
            };
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
            if !distance.is_finite() || distance <= 1.0e-4 {
                continue;
            }
            candidates.push(DiffractionEmitterCandidate {
                emitter_key: entity.stable_u64(),
                position,
                distance,
                blocker_key,
                blocker_entity,
            });
        }
        candidates.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| a.emitter_key.cmp(&b.emitter_key))
        });
        candidates.truncate(MAX_DIFFRACTION_EMITTERS_PER_TICK);
        candidates
    }

    fn blocker_edges(
        &self,
        world: &World,
        blocker_entity: EntityId,
        blocker_key: u64,
    ) -> Arc<[CachedWorldEdge]> {
        let Some(collider) = world.get::<StaticMeshCollider>(blocker_entity) else {
            return Arc::from([]);
        };
        let transform = world
            .get::<Transform>(blocker_entity)
            .copied()
            .unwrap_or_default();
        let revision = collider.runtime_revision(transform);
        if let Some(cached) = self.edge_cache.lock().get(&blocker_key) {
            if cached.revision == revision {
                return Arc::clone(&cached.edges);
            }
        }

        let local = mesh_diffraction_edges(collider.vertices.as_ref(), collider.triangles.as_ref());
        let edges = local
            .into_iter()
            .map(|edge| CachedWorldEdge {
                vertex_indices: edge.vertex_indices,
                endpoints: edge.endpoints.map(|point| {
                    let local = Vec3::new(point[0], point[1], point[2]);
                    let world = transform.rotation * local + transform.position;
                    [world.x, world.y, world.z]
                }),
                wedge_angle_radians: edge.wedge_angle_radians,
            })
            .collect::<Vec<_>>();
        let edges: Arc<[CachedWorldEdge]> = Arc::from(edges.into_boxed_slice());
        self.edge_cache.lock().insert(
            blocker_key,
            CachedBlockerEdges {
                revision,
                edges: Arc::clone(&edges),
            },
        );
        edges
    }

    fn blocker_material(
        world: &World,
        blocker_entity: EntityId,
    ) -> (bool, AcousticMaterialProfile) {
        if let Some(surface) = world.get::<AcousticSurface>(blocker_entity) {
            return (true, surface.clone().sanitized().profile);
        }
        if let Some(surface) = world.get::<PhysicsSurface>(blocker_entity) {
            if let Some(profile) = world
                .resource::<AcousticMaterialLibrary>()
                .and_then(|library| library.resolve(surface.id.as_str()))
                .map(|surface| surface.profile)
            {
                return (true, profile);
            }
        }
        (
            false,
            resolve_acoustic_surface_for_entity(world, Some(blocker_entity)).profile,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn push_leg(
        &self,
        pending: &mut BTreeMap<u64, PendingDiffractionRay>,
        queries: &mut Vec<PhysicsQueryDto>,
        candidate: DiffractionEmitterCandidate,
        listener: Vec3,
        listener_key: Option<u64>,
        edge: CachedWorldEdge,
        geometry: AudioEdgeDiffractionGeometry,
        leg: DiffractionProbeLeg,
        material_known: bool,
        material: AcousticMaterialProfile,
    ) {
        let point = Vec3::new(
            geometry.diffraction_point[0],
            geometry.diffraction_point[1],
            geometry.diffraction_point[2],
        );
        let origin = match leg {
            DiffractionProbeLeg::Source => candidate.position,
            DiffractionProbeLeg::Listener => listener,
        };
        let delta = point - origin;
        let full_length = delta.length();
        if !full_length.is_finite() || full_length <= EDGE_VISIBILITY_ENDPOINT_EPSILON * 2.0 {
            return;
        }
        let dir = delta / full_length;
        let max_t = full_length - EDGE_VISIBILITY_ENDPOINT_EPSILON;
        let seq = self.alloc_query_id();
        pending.insert(
            seq,
            PendingDiffractionRay {
                emitter_key: candidate.emitter_key,
                listener_key,
                blocker_key: candidate.blocker_key,
                leg,
                edge,
                geometry,
                max_t,
                source_position: [
                    candidate.position.x,
                    candidate.position.y,
                    candidate.position.z,
                ],
                listener_position: [listener.x, listener.y, listener.z],
                material_known,
                material,
            },
        );
        queries.push(PhysicsQueryDto {
            seq,
            ignore_entity: match leg {
                DiffractionProbeLeg::Source => Some(candidate.emitter_key),
                DiffractionProbeLeg::Listener => listener_key,
            },
            kind: PhysicsQueryKindDto::Ray {
                origin: [origin.x, origin.y, origin.z],
                dir: [dir.x, dir.y, dir.z],
                max_t,
            },
        });
    }

    fn update_tracking(&self, current: BTreeSet<u64>) {
        let mut tracked = self.tracked_emitters.lock();
        let removed = tracked
            .difference(&current)
            .copied()
            .collect::<BTreeSet<_>>();
        *self.clear_emitters.lock() = removed;
        *tracked = current;
    }
}

impl GameplayPhysicsQueryProvider for AudioDiffractionPhysicsQueryProvider {
    fn id(&self) -> &'static str {
        "engine.audio.physics-diffraction"
    }

    fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto> {
        let Some(listener_state) = world.resource::<AudioListenerRuntimeState>().copied() else {
            self.pending.lock().clear();
            self.update_tracking(BTreeSet::new());
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
            self.update_tracking(BTreeSet::new());
            return Vec::new();
        }

        let mesh_entities = Self::static_mesh_entities(world);
        self.edge_cache
            .lock()
            .retain(|key, _| mesh_entities.contains_key(key));
        let candidates = self.collect_candidates(world, listener, &mesh_entities);
        self.update_tracking(
            candidates
                .iter()
                .map(|candidate| candidate.emitter_key)
                .collect(),
        );
        let listener_key = first_player(world).map(EntityId::stable_u64);
        let mut pending = BTreeMap::new();
        let mut queries = Vec::new();
        for candidate in candidates {
            let edges = self.blocker_edges(world, candidate.blocker_entity, candidate.blocker_key);
            let source = [
                candidate.position.x,
                candidate.position.y,
                candidate.position.z,
            ];
            let receiver = [listener.x, listener.y, listener.z];
            let mut paths = edges
                .iter()
                .filter_map(|edge| {
                    edge_diffraction_geometry(edge.endpoints, source, receiver)
                        .map(|geometry| (*edge, geometry))
                })
                .filter(|(_, geometry)| {
                    geometry.excess_length_m > 1.0e-4 && geometry.bend_angle_radians > 1.0e-3
                })
                .collect::<Vec<_>>();
            paths.sort_by(|a, b| {
                a.1.path_length_m
                    .total_cmp(&b.1.path_length_m)
                    .then_with(|| a.0.vertex_indices.cmp(&b.0.vertex_indices))
            });
            paths.truncate(MAX_EDGE_CANDIDATES_PER_EMITTER);
            let (material_known, material) =
                Self::blocker_material(world, candidate.blocker_entity);
            for (edge, geometry) in paths {
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    candidate,
                    listener,
                    listener_key,
                    edge,
                    geometry,
                    DiffractionProbeLeg::Source,
                    material_known,
                    material,
                );
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    candidate,
                    listener,
                    listener_key,
                    edge,
                    geometry,
                    DiffractionProbeLeg::Listener,
                    material_known,
                    material,
                );
            }
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
        let clear_emitters = std::mem::take(&mut *self.clear_emitters.lock());
        for key in clear_emitters {
            if let Some(entity) = key_to_entity.get(&key).copied() {
                let _ = world.remove::<AudioEdgeDiffractionObservation>(entity);
            }
        }
        if pending.is_empty() {
            return BTreeSet::new();
        }
        let hits_by_seq = hits
            .iter()
            .map(|hit| (hit.seq, *hit))
            .collect::<BTreeMap<_, _>>();
        let mut consumed = BTreeSet::new();
        let mut aggregates = BTreeMap::<(u64, u64, [u32; 2]), DiffractionAggregate>::new();
        for (seq, ray) in pending {
            consumed.insert(seq);
            let blocked = hits_by_seq.get(&seq).is_some_and(|hit| {
                hit.entity != ray.emitter_key
                    && ray.listener_key != Some(hit.entity)
                    && hit.distance.is_finite()
                    && hit.distance >= 0.0
                    && hit.distance <= ray.max_t + 1.0e-4
            });
            let aggregate = aggregates
                .entry((ray.emitter_key, ray.blocker_key, ray.edge.vertex_indices))
                .or_insert_with(|| DiffractionAggregate::new(ray));
            match ray.leg {
                DiffractionProbeLeg::Source => aggregate.source_blocked = blocked,
                DiffractionProbeLeg::Listener => aggregate.listener_blocked = blocked,
            }
        }

        let mut by_emitter = BTreeMap::<
            u64,
            (
                u64,
                [f32; 3],
                [f32; 3],
                Vec<AudioEdgeDiffractionPathObservation>,
            ),
        >::new();
        for ((emitter_key, blocker_key, _), aggregate) in aggregates {
            let entry = by_emitter.entry(emitter_key).or_insert_with(|| {
                (
                    blocker_key,
                    aggregate.source_position,
                    aggregate.listener_position,
                    Vec::new(),
                )
            });
            entry.3.push(AudioEdgeDiffractionPathObservation {
                edge_vertex_indices: aggregate.edge.vertex_indices,
                visible: !aggregate.source_blocked && !aggregate.listener_blocked,
                diffraction_point: aggregate.geometry.diffraction_point,
                arrival_direction: aggregate.geometry.arrival_direction,
                path_length_m: aggregate.geometry.path_length_m,
                excess_length_m: aggregate.geometry.excess_length_m,
                bend_angle_radians: aggregate.geometry.bend_angle_radians,
                wedge_angle_radians: aggregate.edge.wedge_angle_radians,
                material_known: aggregate.material_known,
                material: aggregate.material,
            });
        }

        for (emitter_key, (blocker_key, source_position, listener_position, mut paths)) in
            by_emitter
        {
            let Some(entity) = key_to_entity.get(&emitter_key).copied() else {
                continue;
            };
            paths.sort_by(|a, b| {
                a.path_length_m
                    .total_cmp(&b.path_length_m)
                    .then_with(|| a.edge_vertex_indices.cmp(&b.edge_vertex_indices))
            });
            let _ = world.insert(
                entity,
                AudioEdgeDiffractionObservation {
                    fixed_tick,
                    source_position,
                    listener_position,
                    blocker_entity: Some(blocker_key),
                    paths,
                },
            );
        }
        consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::{AudioListenerState, AudioOcclusionSettings};

    fn cube_collider() -> StaticMeshCollider {
        let vertices = vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        StaticMeshCollider::new(vertices, triangles).expect("cube collider")
    }

    fn world_with_blocker() -> (World, EntityId, EntityId) {
        let mut world = World::new();
        world.insert_resource(AudioListenerRuntimeState {
            listener: AudioListenerState {
                position: [-4.0, 0.0, 0.0],
                ..AudioListenerState::default()
            },
            frame_index: 1,
        });
        let blocker = world.spawn();
        let _ = world.insert(blocker, Transform::default());
        let _ = world.insert(blocker, cube_collider());
        let emitter = world.spawn();
        let _ = world.insert(
            emitter,
            Transform {
                position: Vec3::new(4.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        let mut audio = AudioEmitter::new("shared/audio/test.yscd@edge");
        audio.occlusion = AudioOcclusionSettings::default();
        let _ = world.insert(emitter, audio);
        let _ = world.insert(
            emitter,
            AudioOcclusionObservation {
                fixed_tick: 1,
                samples: 3,
                blocked_samples: 3,
                obstruction: 1.0,
                occlusion: 1.0,
                estimated_thickness_m: 2.0,
                center_blocker_layers: 1,
                dominant_blocker_entity: Some(blocker.stable_u64()),
                dominant_material: "surface.default".to_owned(),
                material: AcousticMaterialProfile::transparent(),
            },
        );
        (world, emitter, blocker)
    }

    fn entity_keys(world: &World) -> BTreeMap<u64, EntityId> {
        world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect()
    }

    #[test]
    fn provider_queries_only_edges_of_the_proven_occlusion_blocker() {
        let (mut world, emitter, blocker) = world_with_blocker();
        let unrelated = world.spawn();
        let _ = world.insert(
            unrelated,
            Transform {
                position: Vec3::new(3.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        let _ = world.insert(unrelated, cube_collider());
        let provider = AudioDiffractionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        assert!(!queries.is_empty());
        assert!(queries.len() <= MAX_EDGE_CANDIDATES_PER_EMITTER * 2);
        assert!(provider
            .pending
            .lock()
            .values()
            .all(|pending| pending.blocker_key == blocker.stable_u64()));
        let keys = entity_keys(&world);
        let consumed = provider.resolve_query_hits(&mut world, 2, &[], &keys);
        assert_eq!(consumed.len(), queries.len());
        let observation = world
            .get::<AudioEdgeDiffractionObservation>(emitter)
            .expect("diffraction observation");
        assert_eq!(observation.blocker_entity, Some(blocker.stable_u64()));
        assert!(observation.paths.iter().all(|path| path.visible));
    }

    #[test]
    fn one_blocked_visibility_leg_closes_only_its_edge_candidate() {
        let (mut world, emitter, _) = world_with_blocker();
        let obstacle = world.spawn();
        let provider = AudioDiffractionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let query = queries.first().expect("diffraction query");
        let max_t = match query.kind {
            PhysicsQueryKindDto::Ray { max_t, .. } => max_t,
            _ => panic!("ray expected"),
        };
        let hit = PhysicsQueryHitDto {
            seq: query.seq,
            entity: obstacle.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: max_t * 0.5,
        };
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 3, &[hit], &keys);
        let observation = world
            .get::<AudioEdgeDiffractionObservation>(emitter)
            .expect("diffraction observation");
        assert_eq!(
            observation
                .paths
                .iter()
                .filter(|path| !path.visible)
                .count(),
            1
        );
        assert!(observation.paths.iter().any(|path| path.visible));
    }
}
