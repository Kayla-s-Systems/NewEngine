#![forbid(unsafe_op_in_unsafe_fn)]

//! Scene-side material application adapter.
//!
//! Materials are parsed/registered by `newengine-materials`. This module is the
//! only place in the scene bridge that turns a material id into ECS components on
//! an object. That keeps material authoring independent from terrain/game/demo
//! bootstrap code and makes the future authoring inspector use the same path.

use newengine_ecs::EntityId;
use newengine_materials::{
    MaterialDomain, MaterialId, MaterialOverrides, MaterialRef, MaterialRegistry, ShadingModel,
};

use super::imported_assets::PrimitiveMaterialBase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialApplyMode {
    /// Apply exactly this material asset/instance id.
    Exact,
    /// Create/update a deterministic per-entity primitive instance with a color override.
    PrimitiveTint,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MaterialApplySpec {
    pub material: MaterialId,
    pub fallback: MaterialId,
    pub color: [f32; 4],
    pub mode: MaterialApplyMode,
}

impl MaterialApplySpec {
    #[inline]
    pub fn exact(material: MaterialId, fallback: MaterialId, color: [f32; 4]) -> Self {
        Self {
            material,
            fallback,
            color,
            mode: MaterialApplyMode::Exact,
        }
    }

    #[inline]
    pub fn primitive_tint(material: MaterialId, fallback: MaterialId, color: [f32; 4]) -> Self {
        Self {
            material,
            fallback,
            color,
            mode: MaterialApplyMode::PrimitiveTint,
        }
    }

    #[inline]
    pub fn effective_material(self) -> MaterialId {
        if self.material.is_valid() {
            self.material
        } else {
            self.fallback
        }
    }
}

#[inline]
pub(super) fn apply_material_to_entity(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    spec: MaterialApplySpec,
) -> MaterialId {
    let base = spec.effective_material();
    let _ = world.insert(entity, PrimitiveMaterialBase { id: base });

    let applied = match spec.mode {
        MaterialApplyMode::Exact => base,
        MaterialApplyMode::PrimitiveTint => {
            let inst_name = format!("__prim_{:016x}", entity.stable_u64());
            let overrides = MaterialOverrides {
                domain: Some(MaterialDomain::Surface),
                shading_model: Some(ShadingModel::Unlit),
                base_color: Some(spec.color),
                ..MaterialOverrides::default()
            };
            mats.upsert_instance_named(base, &inst_name, overrides)
        }
    };

    let _ = world.insert(entity, MaterialRef { id: applied });
    applied
}
