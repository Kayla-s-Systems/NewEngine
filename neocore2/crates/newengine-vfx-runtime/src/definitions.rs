use std::collections::{BTreeMap, VecDeque};

use newengine_math::Vec3;
use newengine_primitives::PrimitiveId;
use newengine_vfx_api::{
    VfxBudgetV1, VfxEffectRef, VfxGpuBillboardMode, VfxPriority, VfxSpawnRequestV1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VfxInstanceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VfxLayerKind {
    MuzzleFlash,
    MuzzleCore,
    Smoke,
    Tracer,
    Spark,
    ImpactDecal,
    Trail,
    Debris,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxRenderRole {
    Transparent,
    Decal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxAlignment {
    None,
    DirectionY,
    DirectionZ,
    NormalY,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VfxEmissionAxis {
    #[default]
    Normal,
    Direction,
    Reflection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLightDefinition {
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VfxLayerDefinition {
    Pulse {
        kind: VfxLayerKind,
        primitive: PrimitiveId,
        role: VfxRenderRole,
        alignment: VfxAlignment,
        texture_slot: u8,
        billboard: VfxGpuBillboardMode,
        offset_along_direction: f32,
        offset_along_normal: f32,
        scale: Vec3,
        growth_per_second: Vec3,
        color: [f32; 4],
        lifetime_seconds: f32,
        fade_start_fraction: f32,
        fade_in_fraction: f32,
        drag_per_second: f32,
        depth_softness_m: f32,
        rotation_radians: f32,
        rotation_random_radians: f32,
        spin_radians_per_second: f32,
        light: Option<VfxLightDefinition>,
    },
    Tracer {
        primitive: PrimitiveId,
        color: [f32; 4],
        mode: VfxTracerMode,
        half_length: f32,
        radius: f32,
        speed: f32,
        max_lifetime_seconds: f32,
    },
    Burst {
        kind: VfxLayerKind,
        primitive: PrimitiveId,
        role: VfxRenderRole,
        texture_slot: u8,
        billboard: VfxGpuBillboardMode,
        emission_axis: VfxEmissionAxis,
        count: u16,
        scale: Vec3,
        color: [f32; 4],
        speed_min: f32,
        speed_max: f32,
        cone_angle_degrees: f32,
        size_variance: f32,
        lifetime_variance: f32,
        acceleration: Vec3,
        drag_per_second: f32,
        depth_softness_m: f32,
        rotation_random_radians: f32,
        spin_radians_per_second: f32,
        spin_variance: f32,
        lifetime_seconds: f32,
        fade_start_fraction: f32,
        fade_in_fraction: f32,
    },
    Decal {
        primitive: PrimitiveId,
        material_ref: Option<String>,
        scale: Vec3,
        color: [f32; 4],
        normal_offset: f32,
        persistent: bool,
        lifetime_seconds: f32,
        fade_start_fraction: f32,
    },
}

impl VfxLayerDefinition {
    #[inline]
    pub fn estimated_layers(&self) -> u32 {
        match self {
            Self::Burst { count, .. } => u32::from(*count),
            _ => 1,
        }
    }

    #[inline]
    pub fn estimated_particles(&self) -> u32 {
        match self {
            Self::Burst { count, .. } => u32::from(*count),
            Self::Pulse { .. } | Self::Tracer { .. } | Self::Decal { .. } => 1,
        }
    }

    #[inline]
    pub fn estimated_lights(&self) -> u32 {
        match self {
            Self::Pulse { light: Some(_), .. } => 1,
            _ => 0,
        }
    }

    #[inline]
    pub fn estimated_decals(&self) -> u32 {
        u32::from(matches!(self, Self::Decal { .. }))
    }

    pub fn max_lifetime_seconds(&self) -> f32 {
        match self {
            Self::Pulse {
                lifetime_seconds, ..
            }
            | Self::Burst {
                lifetime_seconds, ..
            }
            | Self::Decal {
                lifetime_seconds,
                persistent: false,
                ..
            } => *lifetime_seconds,
            Self::Decal {
                persistent: true, ..
            } => 0.0,
            Self::Tracer {
                max_lifetime_seconds,
                ..
            } => *max_lifetime_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VfxEffectDefinition {
    pub effect: VfxEffectRef,
    pub priority: VfxPriority,
    pub layers: Vec<VfxLayerDefinition>,
}

impl VfxEffectDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.effect.0.trim().is_empty() {
            return Err("VFX effect definition requires a non-empty id".to_owned());
        }
        if self.layers.is_empty() {
            return Err(format!(
                "VFX '{}' requires at least one layer",
                self.effect.0
            ));
        }
        if self.layers.iter().any(|layer| {
            if matches!(
                layer,
                VfxLayerDefinition::Decal {
                    persistent: true,
                    ..
                }
            ) {
                return false;
            }
            !layer.max_lifetime_seconds().is_finite() || layer.max_lifetime_seconds() <= 0.0
        }) {
            return Err(format!(
                "VFX '{}' contains invalid layer lifetime",
                self.effect.0
            ));
        }
        Ok(())
    }

    #[inline]
    pub fn estimated_layers(&self) -> u32 {
        self.layers
            .iter()
            .map(VfxLayerDefinition::estimated_layers)
            .sum()
    }

    #[inline]
    pub fn estimated_particles(&self) -> u32 {
        self.layers
            .iter()
            .map(VfxLayerDefinition::estimated_particles)
            .sum()
    }

    #[inline]
    pub fn estimated_lights(&self) -> u32 {
        self.layers
            .iter()
            .map(VfxLayerDefinition::estimated_lights)
            .sum()
    }

    #[inline]
    pub fn estimated_decals(&self) -> u32 {
        self.layers
            .iter()
            .map(VfxLayerDefinition::estimated_decals)
            .sum()
    }

    pub fn max_lifetime_seconds(&self) -> f32 {
        self.layers
            .iter()
            .map(VfxLayerDefinition::max_lifetime_seconds)
            .fold(0.0, f32::max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VfxSurfaceResponse {
    pub spark_color: Option<[f32; 3]>,
    pub spark_alpha_scale: f32,
    pub smoke_color: Option<[f32; 3]>,
    pub decal_color: Option<[f32; 3]>,
}

impl VfxSurfaceResponse {
    pub const fn authored(
        spark_color: Option<[f32; 3]>,
        spark_alpha_scale: f32,
        smoke_color: Option<[f32; 3]>,
        decal_color: Option<[f32; 3]>,
    ) -> Self {
        Self {
            spark_color,
            spark_alpha_scale,
            smoke_color,
            decal_color,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VfxSurfaceResponseLibrary {
    responses: BTreeMap<String, VfxSurfaceResponse>,
}

impl Default for VfxSurfaceResponseLibrary {
    fn default() -> Self {
        let mut responses = BTreeMap::new();
        responses.insert(
            "surface.default".to_owned(),
            VfxSurfaceResponse::authored(None, 1.0, None, None),
        );
        Self { responses }
    }
}

impl VfxSurfaceResponseLibrary {
    pub fn register(
        &mut self,
        surface: impl AsRef<str>,
        response: VfxSurfaceResponse,
    ) -> Result<(), String> {
        let surface = surface.as_ref().trim().to_ascii_lowercase();
        if surface.is_empty() {
            return Err("VFX surface response requires a non-empty surface id".to_owned());
        }
        if !response.spark_alpha_scale.is_finite() || response.spark_alpha_scale < 0.0 {
            return Err(format!(
                "VFX surface '{}' has invalid spark alpha scale",
                surface
            ));
        }
        self.responses.insert(surface, response);
        Ok(())
    }

    pub fn resolve(&self, surface: Option<&str>) -> VfxSurfaceResponse {
        surface
            .and_then(|surface| self.responses.get(&surface.trim().to_ascii_lowercase()))
            .copied()
            .or_else(|| self.responses.get("surface.default").copied())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
pub struct VfxEffectLibrary {
    effects: BTreeMap<String, VfxEffectDefinition>,
}

impl Default for VfxEffectLibrary {
    fn default() -> Self {
        Self {
            effects: BTreeMap::new(),
        }
    }
}

impl VfxEffectLibrary {
    pub fn register(&mut self, mut definition: VfxEffectDefinition) -> Result<(), String> {
        definition.effect.0 = definition.effect.0.trim().to_owned();
        definition.validate()?;
        if self.effects.contains_key(&definition.effect.0) {
            return Err(format!(
                "VFX effect already registered: {}",
                definition.effect.0
            ));
        }
        self.effects.insert(definition.effect.0.clone(), definition);
        Ok(())
    }

    #[inline]
    pub fn get(&self, effect: &str) -> Option<&VfxEffectDefinition> {
        self.effects.get(effect.trim())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VfxTracerMode {
    #[default]
    Swept,
    SingleFrame,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxInstanceRoot {
    pub id: VfxInstanceId,
    pub owner_stable_id: Option<u64>,
    pub correlation_id: u64,
    pub remaining_seconds: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxDecalMaterialAssetRef {
    pub logical_ref: String,
}

/// Marker for an authored world-persistent decal. Persistent decals are intentionally not owned by
/// `VfxLayerRuntime`: transient instance teardown and age-based fades must never remove them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxPersistentDecal {
    pub source_instance_id: VfxInstanceId,
    pub owner_stable_id: Option<u64>,
    pub correlation_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLayerRuntime {
    pub instance_id: VfxInstanceId,
    pub owner_stable_id: Option<u64>,
    pub correlation_id: u64,
    pub kind: VfxLayerKind,
    pub tracer_mode: VfxTracerMode,
    /// Single-frame tracers are admitted during pre-update, survive that same update for render,
    /// then retire on the next update without any frame-rate-derived lifetime heuristic.
    pub tracer_updates_remaining: u8,
    pub origin: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub age_seconds: f32,
    pub lifetime_seconds: f32,
    pub base_scale: Vec3,
    pub growth_per_second: Vec3,
    pub start_color: [f32; 4],
    pub fade_start_fraction: f32,
    pub traveled: f32,
    pub max_distance: f32,
    pub initial_light_intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxGpuLayerRuntime {
    pub instance_id: VfxInstanceId,
    pub kind: VfxLayerKind,
    pub particle_count: u32,
    pub remaining_seconds: f32,
}

#[derive(Clone, Debug, Default)]
pub struct VfxGpuParticleLedger {
    layers: Vec<VfxGpuLayerRuntime>,
}

impl VfxGpuParticleLedger {
    #[inline]
    pub fn layers(&self) -> &[VfxGpuLayerRuntime] {
        &self.layers
    }

    pub(crate) fn push(&mut self, layer: VfxGpuLayerRuntime) {
        if layer.particle_count > 0 && layer.remaining_seconds > 0.0 {
            self.layers.push(layer);
        }
    }

    pub(crate) fn step(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.remaining_seconds -= dt;
        }
        self.layers.retain(|layer| layer.remaining_seconds > 0.0);
    }

    pub(crate) fn remove_instance(&mut self, id: VfxInstanceId) -> u32 {
        let mut removed = 0u32;
        self.layers.retain(|layer| {
            if layer.instance_id == id {
                removed = removed.saturating_add(layer.particle_count);
                false
            } else {
                true
            }
        });
        removed
    }
}

pub const DEFAULT_VFX_SPAWN_QUEUE_CAPACITY: usize = 2_048;

/// Bounded deterministic queue for semantic VFX spawn requests.
///
/// Requests are normalized before admission. Overflow rejects the newest request so
/// already-admitted frame work retains stable FIFO ordering.
#[derive(Clone, Debug)]
pub struct VfxSpawnQueue {
    capacity: usize,
    pending: VecDeque<VfxSpawnRequestV1>,
    dropped_requests: u64,
}

impl Default for VfxSpawnQueue {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_VFX_SPAWN_QUEUE_CAPACITY)
    }
}

impl VfxSpawnQueue {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, 65_536),
            pending: VecDeque::new(),
            dropped_requests: 0,
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    #[inline]
    pub fn dropped_requests(&self) -> u64 {
        self.dropped_requests
    }

    pub(crate) fn push_normalized(&mut self, request: VfxSpawnRequestV1) -> bool {
        if self.pending.len() >= self.capacity {
            self.dropped_requests = self.dropped_requests.saturating_add(1);
            return false;
        }
        self.pending.push_back(request);
        true
    }

    #[inline]
    pub(crate) fn pop_front(&mut self) -> Option<VfxSpawnRequestV1> {
        self.pending.pop_front()
    }

    #[inline]
    pub(crate) fn note_execution_drop(&mut self) {
        self.dropped_requests = self.dropped_requests.saturating_add(1);
    }
    pub(crate) fn clamp_pending_to_point(
        &mut self,
        owner_stable_id: u64,
        correlation_id: u64,
        point: Vec3,
    ) -> usize {
        let mut clamped = 0usize;
        for request in &mut self.pending {
            if request.owner.map(|owner| owner.stable_id) != Some(owner_stable_id)
                || request.correlation_id != correlation_id
                || request.max_distance <= 0.0
            {
                continue;
            }
            let origin = Vec3::new(
                request.position[0],
                request.position[1],
                request.position[2],
            );
            let hit_distance = (point - origin).length();
            if hit_distance.is_finite() {
                request.max_distance = request.max_distance.min(hit_distance.max(0.0));
                clamped += 1;
            }
        }
        clamped
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VfxQueueProcessReport {
    pub processed: u32,
    pub spawned: u32,
    pub budget_rejected: u32,
    pub failed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VfxRuntimeStage {
    #[default]
    Idle,
    PreUpdate,
    Update,
    AfterPreRender,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxRuntimeState {
    pub budget: VfxBudgetV1,
    pub stage: VfxRuntimeStage,
    pub frame_index: u64,
    pub spawned_instances: u64,
    pub dropped_instances: u64,
    pub dropped_layers: u64,
    next_instance_id: u64,
}

impl Default for VfxRuntimeState {
    fn default() -> Self {
        Self {
            budget: VfxBudgetV1::default(),
            stage: VfxRuntimeStage::Idle,
            frame_index: 0,
            spawned_instances: 0,
            dropped_instances: 0,
            dropped_layers: 0,
            next_instance_id: 0,
        }
    }
}

impl VfxRuntimeState {
    pub fn with_budget(budget: VfxBudgetV1) -> Self {
        Self {
            budget: budget.sanitized(),
            ..Self::default()
        }
    }

    pub(crate) fn allocate_instance_id(&mut self) -> VfxInstanceId {
        self.next_instance_id = self.next_instance_id.wrapping_add(1).max(1);
        VfxInstanceId(self.next_instance_id)
    }
}
