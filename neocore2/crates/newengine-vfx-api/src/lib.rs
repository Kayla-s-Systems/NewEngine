#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral semantic VFX contracts.
//!
//! Gameplay publishes effect intent. The VFX runtime owns composition, budgets,
//! lifetime, LOD/culling policy and renderer-facing realization.

mod fxd;
mod gpu_particles;
pub use fxd::*;
pub use gpu_particles::*;

pub use newengine_entity_api::EntityHandle;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ENGINE_VFX_SERVICE_ID: &str = "engine.render.vfx";
pub const VFX_SERVICE_ID: &str = "render.vfx.api";
pub const VFX_BACKEND_CAPABILITY_ID: &str = "render.vfx.particles";
pub const VFX_RUNTIME_CONTRACT: &str = "newengine.vfx-api/v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VfxEffectRef(pub String);

impl VfxEffectRef {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VfxDomain {
    #[default]
    Composite,
    Particle,
    Decal,
    Trail,
    Light,
    Explosion,
    Fire,
    Liquid,
    Weather,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum VfxPriority {
    Critical,
    High,
    #[default]
    Normal,
    Low,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VfxSpawnRequestV1 {
    pub effect: VfxEffectRef,
    pub owner: Option<EntityHandle>,
    pub correlation_id: u64,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub normal: [f32; 3],
    pub velocity: [f32; 3],
    pub scale: f32,
    pub intensity: f32,
    pub max_distance: f32,
    pub lifetime_seconds: Option<f32>,
    pub seed: u64,
    pub surface: Option<String>,
    pub tags: Vec<String>,
}

impl Default for VfxSpawnRequestV1 {
    fn default() -> Self {
        Self {
            effect: VfxEffectRef::new(String::new()),
            owner: None,
            correlation_id: 0,
            position: [0.0; 3],
            direction: [0.0, 0.0, -1.0],
            normal: [0.0, 1.0, 0.0],
            velocity: [0.0; 3],
            scale: 1.0,
            intensity: 1.0,
            max_distance: 0.0,
            lifetime_seconds: None,
            seed: 0,
            surface: None,
            tags: Vec::new(),
        }
    }
}

impl VfxSpawnRequestV1 {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.effect.0 = self.effect.0.trim().to_owned();
        if self.effect.0.is_empty() || self.effect.0.len() > 256 {
            return Err("VFX effect id must contain 1..=256 bytes".to_owned());
        }
        if !self.position.iter().all(|value| value.is_finite()) {
            return Err(format!(
                "VFX '{}' position contains non-finite data",
                self.effect.0
            ));
        }
        self.direction = normalized_or(self.direction, [0.0, 0.0, -1.0]);
        self.normal = normalized_or(self.normal, [0.0, 1.0, 0.0]);
        self.velocity = finite_vec_or(self.velocity, [0.0; 3]);
        self.scale = finite_or(self.scale, 1.0).clamp(0.001, 1_000.0);
        self.intensity = finite_or(self.intensity, 1.0).clamp(0.0, 1_000.0);
        self.max_distance = finite_or(self.max_distance, 0.0).clamp(0.0, 1_000_000.0);
        self.lifetime_seconds = self
            .lifetime_seconds
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| value.clamp(0.001, 3_600.0));
        self.surface = self.surface.and_then(|value| {
            let value = value.trim().to_ascii_lowercase();
            (!value.is_empty()).then_some(value)
        });
        self.tags = self
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VfxBudgetV1 {
    pub max_active_instances: u32,
    pub max_active_layers: u32,
    pub max_transient_lights: u32,
    pub max_decals: u32,
    pub max_trails: u32,
    pub max_particle_estimate: u32,
}

impl Default for VfxBudgetV1 {
    fn default() -> Self {
        Self {
            max_active_instances: 512,
            max_active_layers: 4_096,
            max_transient_lights: 64,
            max_decals: 512,
            max_trails: 128,
            max_particle_estimate: 262_144,
        }
    }
}

impl VfxBudgetV1 {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            max_active_instances: self.max_active_instances.clamp(1, 65_536),
            max_active_layers: self.max_active_layers.clamp(1, 1_000_000),
            max_transient_lights: self.max_transient_lights.clamp(0, 16_384),
            max_decals: self.max_decals.clamp(0, 1_000_000),
            max_trails: self.max_trails.clamp(0, 65_536),
            max_particle_estimate: self.max_particle_estimate.clamp(1, 16_777_216),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VfxRuntimeStatsV1 {
    pub active_instances: u32,
    pub active_layers: u32,
    pub transient_lights: u32,
    pub decals: u32,
    pub trails: u32,
    pub particle_estimate: u32,
    pub spawned_instances: u64,
    pub pending_requests: u32,
    pub dropped_requests: u64,
    pub dropped_instances: u64,
    pub dropped_layers: u64,
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn finite_vec_or(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    if value.iter().all(|component| component.is_finite()) {
        value
    } else {
        fallback
    }
}

fn normalized_or(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    if !value.iter().all(|component| component.is_finite()) {
        return fallback;
    }
    let length_sq = value
        .iter()
        .map(|component| component * component)
        .sum::<f32>();
    if length_sq <= 1.0e-12 || !length_sq.is_finite() {
        return fallback;
    }
    let inv_length = length_sq.sqrt().recip();
    [
        value[0] * inv_length,
        value[1] * inv_length,
        value[2] * inv_length,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_request_normalizes_vectors_surface_and_tags() {
        let request = VfxSpawnRequestV1 {
            effect: VfxEffectRef::new(" vfx.weapon.impact.default "),
            direction: [0.0; 3],
            normal: [0.0, 4.0, 0.0],
            surface: Some(" Surface.Metal ".to_owned()),
            tags: vec![" Impact ".to_owned(), "impact".to_owned()],
            ..Default::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(request.effect.as_str(), "vfx.weapon.impact.default");
        assert_eq!(request.direction, [0.0, 0.0, -1.0]);
        assert_eq!(request.normal, [0.0, 1.0, 0.0]);
        assert_eq!(request.surface.as_deref(), Some("surface.metal"));
        assert_eq!(request.tags, vec!["impact"]);
    }

    #[test]
    fn invalid_position_is_rejected_before_runtime_spawn() {
        let request = VfxSpawnRequestV1 {
            effect: VfxEffectRef::new("vfx.test"),
            position: [f32::NAN, 0.0, 0.0],
            ..Default::default()
        };
        assert!(request.normalized().is_err());
    }
}
