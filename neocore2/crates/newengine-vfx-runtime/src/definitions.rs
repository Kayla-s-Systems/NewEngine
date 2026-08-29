use std::collections::{BTreeMap, VecDeque};

use newengine_math::Vec3;
use newengine_primitives::{builtins, PrimitiveId};
use newengine_vfx_api::{VfxBudgetV1, VfxEffectRef, VfxPriority, VfxSpawnRequestV1};

pub const VFX_WEAPON_SHOT_DEFAULT: &str = "vfx.weapon.shot.default";
pub const VFX_WEAPON_IMPACT_DEFAULT: &str = "vfx.weapon.impact.default";

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
        offset_along_direction: f32,
        offset_along_normal: f32,
        scale: Vec3,
        growth_per_second: Vec3,
        color: [f32; 4],
        lifetime_seconds: f32,
        fade_start_fraction: f32,
        light: Option<VfxLightDefinition>,
    },
    Tracer {
        primitive: PrimitiveId,
        color: [f32; 4],
        half_length: f32,
        radius: f32,
        speed: f32,
        max_lifetime_seconds: f32,
    },
    Burst {
        kind: VfxLayerKind,
        primitive: PrimitiveId,
        role: VfxRenderRole,
        count: u16,
        scale: Vec3,
        color: [f32; 4],
        speed_min: f32,
        speed_max: f32,
        acceleration: Vec3,
        lifetime_seconds: f32,
        fade_start_fraction: f32,
    },
    Decal {
        primitive: PrimitiveId,
        scale: Vec3,
        color: [f32; 4],
        normal_offset: f32,
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
                lifetime_seconds, ..
            } => *lifetime_seconds,
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
        let mut library = Self {
            responses: BTreeMap::new(),
        };
        library.responses.insert(
            "surface.default".to_owned(),
            VfxSurfaceResponse::authored(None, 1.0, None, None),
        );
        library.responses.insert(
            "surface.metal".to_owned(),
            VfxSurfaceResponse::authored(
                Some([1.0, 0.78, 0.34]),
                1.0,
                None,
                Some([0.055, 0.050, 0.045]),
            ),
        );
        library.responses.insert(
            "surface.wood".to_owned(),
            VfxSurfaceResponse::authored(
                Some([0.84, 0.48, 0.16]),
                0.72,
                Some([0.30, 0.23, 0.17]),
                Some([0.11, 0.070, 0.035]),
            ),
        );
        let concrete = VfxSurfaceResponse::authored(
            Some([0.92, 0.70, 0.42]),
            0.62,
            Some([0.38, 0.37, 0.35]),
            None,
        );
        library
            .responses
            .insert("surface.concrete".to_owned(), concrete);
        library
            .responses
            .insert("surface.stone".to_owned(), concrete);
        library
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
        let mut library = Self {
            effects: BTreeMap::new(),
        };
        library
            .register(default_weapon_shot_definition())
            .expect("built-in weapon shot VFX must validate");
        library
            .register(default_weapon_impact_definition())
            .expect("built-in weapon impact VFX must validate");
        library
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxInstanceRoot {
    pub id: VfxInstanceId,
    pub owner_stable_id: Option<u64>,
    pub correlation_id: u64,
    pub remaining_seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLayerRuntime {
    pub instance_id: VfxInstanceId,
    pub owner_stable_id: Option<u64>,
    pub correlation_id: u64,
    pub kind: VfxLayerKind,
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

fn default_weapon_shot_definition() -> VfxEffectDefinition {
    VfxEffectDefinition {
        effect: VfxEffectRef::new(VFX_WEAPON_SHOT_DEFAULT),
        priority: VfxPriority::High,
        layers: vec![
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::MuzzleFlash,
                primitive: builtins::ID_CONE,
                role: VfxRenderRole::Transparent,
                alignment: VfxAlignment::DirectionY,
                offset_along_direction: 0.085,
                offset_along_normal: 0.0,
                scale: Vec3::new(0.075, 0.17, 0.075),
                growth_per_second: Vec3::new(0.12, 0.30, 0.12),
                color: [1.0, 0.50, 0.08, 0.92],
                lifetime_seconds: 0.045,
                fade_start_fraction: 0.15,
                light: None,
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::MuzzleCore,
                primitive: builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent,
                alignment: VfxAlignment::None,
                offset_along_direction: 0.018,
                offset_along_normal: 0.0,
                scale: Vec3::new(0.038, 0.038, 0.060),
                growth_per_second: Vec3::new(0.05, 0.05, 0.08),
                color: [1.0, 0.88, 0.48, 1.0],
                lifetime_seconds: 0.032,
                fade_start_fraction: 0.08,
                light: Some(VfxLightDefinition {
                    color: [1.0, 0.55, 0.16],
                    intensity: 34.0,
                    range: 3.8,
                }),
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::Smoke,
                primitive: builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent,
                alignment: VfxAlignment::None,
                offset_along_direction: 0.10,
                offset_along_normal: 0.025,
                scale: Vec3::new(0.035, 0.028, 0.070),
                growth_per_second: Vec3::new(0.28, 0.34, 0.46),
                color: [0.24, 0.22, 0.20, 0.32],
                lifetime_seconds: 0.55,
                fade_start_fraction: 0.18,
                light: None,
            },
            VfxLayerDefinition::Tracer {
                primitive: builtins::ID_CUBE,
                color: [1.0, 0.72, 0.24, 0.90],
                half_length: 0.12,
                radius: 0.004,
                speed: 320.0,
                max_lifetime_seconds: 0.8,
            },
        ],
    }
}

fn default_weapon_impact_definition() -> VfxEffectDefinition {
    VfxEffectDefinition {
        effect: VfxEffectRef::new(VFX_WEAPON_IMPACT_DEFAULT),
        priority: VfxPriority::High,
        layers: vec![
            VfxLayerDefinition::Burst {
                kind: VfxLayerKind::Spark,
                primitive: builtins::ID_CUBE,
                role: VfxRenderRole::Transparent,
                count: 8,
                scale: Vec3::new(0.006, 0.006, 0.045),
                color: [1.0, 0.64, 0.18, 0.95],
                speed_min: 3.5,
                speed_max: 9.0,
                acceleration: Vec3::new(0.0, -6.5, 0.0),
                lifetime_seconds: 0.22,
                fade_start_fraction: 0.30,
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::Smoke,
                primitive: builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent,
                alignment: VfxAlignment::NormalY,
                offset_along_direction: 0.0,
                offset_along_normal: 0.025,
                scale: Vec3::new(0.045, 0.025, 0.045),
                growth_per_second: Vec3::new(0.42, 0.28, 0.42),
                color: [0.26, 0.25, 0.24, 0.34],
                lifetime_seconds: 0.72,
                fade_start_fraction: 0.12,
                light: None,
            },
            VfxLayerDefinition::Decal {
                primitive: builtins::ID_DISC,
                scale: Vec3::new(0.16, 0.002, 0.16),
                color: [0.08, 0.065, 0.055, 0.88],
                normal_offset: 0.003,
                lifetime_seconds: 18.0,
                fade_start_fraction: 0.82,
            },
        ],
    }
}
