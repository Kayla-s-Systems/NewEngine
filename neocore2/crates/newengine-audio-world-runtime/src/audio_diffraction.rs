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
use newengine_physics_world_api::{
    GameplayPhysicsQueryProvider, PhysicsSurface, StaticMeshCollider,
};

const AUDIO_DIFFRACTION_QUERY_NAMESPACE: u64 = 0xa0d0_0000_0000_0000;
const AUDIO_DIFFRACTION_QUERY_COUNTER_MASK: u64 = 0x000f_ffff_ffff_ffff;
const MAX_DIFFRACTION_EMITTERS_PER_TICK: usize = 8;
const MAX_EDGE_CANDIDATES_PER_EMITTER: usize = 6;
/// Only a bounded set of edges nearest the blocked direct path proceeds to the expensive
/// diffraction geometry solve. This keeps collider complexity out of the fixed-tick budget.
const MAX_EDGE_GEOMETRY_PREFILTER: usize = 24;
/// Edge diffraction is a secondary acoustic field, not a rigid-body control signal.
/// Sampling it at 10 Hz preserves perceptual continuity while avoiding a full blocker-edge
/// geometry solve on every 60 Hz physics tick. The latest observation remains authoritative
/// between samples; direct occlusion continues to update every fixed tick.
const DIFFRACTION_QUERY_INTERVAL_TICKS: u64 = 6;
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

include!("audio_diffraction/provider.rs");
include!("audio_diffraction/queries.rs");
include!("audio_diffraction/geometry.rs");
include!("audio_diffraction/tests.rs");
