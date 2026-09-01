use newengine_ecs::{EntityId, World};
use newengine_lighting::PointLight;
use newengine_math::{Quat, Vec3};
use newengine_model_domain_api::{
    MeshCullPolicy, MeshDepthPolicy, MeshRenderOptions, MeshRenderRole, MeshShadowPolicy,
    MeshSortPolicy,
};
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_transform::Transform;
use newengine_vfx_api::{
    VfxGpuParticleBridge, VfxGpuParticleKind, VfxGpuParticleSpawnV1, VfxGpuTextureRegistry,
    VfxRuntimeStatsV1, VfxSpawnRequestV1,
};

use crate::{
    VfxAlignment, VfxDecalMaterialAssetRef, VfxEffectLibrary, VfxEmissionAxis, VfxGpuLayerRuntime,
    VfxGpuParticleLedger, VfxInstanceId, VfxInstanceRoot, VfxLayerDefinition, VfxLayerKind,
    VfxLayerRuntime, VfxLightDefinition, VfxPersistentDecal, VfxQueueProcessReport, VfxRenderRole,
    VfxRuntimeStage, VfxRuntimeState, VfxSpawnQueue, VfxSurfaceResponse, VfxSurfaceResponseLibrary,
    VfxTracerMode,
};

#[derive(Clone, Copy)]
struct VfxSpawnContext<'a> {
    instance_id: VfxInstanceId,
    owner_stable_id: Option<u64>,
    request: &'a VfxSpawnRequestV1,
    requested_lifetime: Option<f32>,
    surface_response: VfxSurfaceResponse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VfxTransientLightRuntime {
    instance_id: VfxInstanceId,
    age_seconds: f32,
    lifetime_seconds: f32,
    fade_start_fraction: f32,
    initial_intensity: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct LiveCounts {
    instances: u32,
    layers: u32,
    lights: u32,
    decals: u32,
    trails: u32,
    particles: u32,
}

// Runtime slices remain in one private module; the crate's exported surface is unchanged.
include!("runtime/lifecycle.rs");
include!("runtime/layer_spawn.rs");
include!("runtime/render_helpers.rs");
include!("runtime/accounting.rs");
include!("runtime/math.rs");
