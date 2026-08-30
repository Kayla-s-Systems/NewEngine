#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialRef, MaterialResolved};
use newengine_math::collections::{FxHashMap, FxHashSet};

use super::material_bindings::LitMaterialPlan;

/// Owned material DTO retained across passes and frames.
///
/// Texture references use `Arc<str>` so cache hits do not clone path strings and
/// the borrowed render-plan adapter remains independent from registry lock lifetimes.
#[derive(Clone, Debug)]
pub(super) struct OwnedLitMaterialPlan {
    base_color: [f32; 4],
    emissive_radiance: [f32; 3],
    alpha_cutoff: f32,
    uv_transform: [f32; 4],
    material_params: [f32; 4],
    base_color_texture: Option<Arc<str>>,
    normal_texture: Option<Arc<str>>,
    roughness_texture: Option<Arc<str>>,
    double_sided: bool,
    alpha_blend: bool,
    cast_shadows: bool,
    receive_shadows: bool,
}

impl OwnedLitMaterialPlan {
    fn from_resolved(material: &MaterialResolved) -> Self {
        Self::from_borrowed(LitMaterialPlan::from_resolved(
            Some(material),
            material.desc.base_color,
        ))
    }

    fn fallback(fallback_color: [f32; 4]) -> Self {
        Self::from_borrowed(LitMaterialPlan::from_resolved(None, fallback_color))
    }

    fn from_borrowed(plan: LitMaterialPlan<'_>) -> Self {
        Self {
            base_color: plan.base_color,
            emissive_radiance: plan.emissive_radiance,
            alpha_cutoff: plan.alpha_cutoff,
            uv_transform: plan.uv_transform,
            material_params: plan.material_params,
            base_color_texture: plan.base_color_texture.map(Arc::<str>::from),
            normal_texture: plan.normal_texture.map(Arc::<str>::from),
            roughness_texture: plan.roughness_texture.map(Arc::<str>::from),
            double_sided: plan.double_sided,
            alpha_blend: plan.alpha_blend,
            cast_shadows: plan.cast_shadows,
            receive_shadows: plan.receive_shadows,
        }
    }

    pub(super) fn as_borrowed(&self) -> LitMaterialPlan<'_> {
        LitMaterialPlan {
            base_color: self.base_color,
            emissive_radiance: self.emissive_radiance,
            alpha_cutoff: self.alpha_cutoff,
            uv_transform: self.uv_transform,
            material_params: self.material_params,
            base_color_texture: self.base_color_texture.as_deref(),
            normal_texture: self.normal_texture.as_deref(),
            roughness_texture: self.roughness_texture.as_deref(),
            double_sided: self.double_sided,
            alpha_blend: self.alpha_blend,
            cast_shadows: self.cast_shadows,
            receive_shadows: self.receive_shadows,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ResolvedLitMaterialPlanCacheStats {
    pub(super) entries: usize,
    pub(super) unresolved_entries: usize,
    pub(super) hits: u64,
    pub(super) negative_hits: u64,
    pub(super) misses: u64,
    pub(super) invalidations: u64,
}

/// Registry-revision-aware cache of owned resolved lit material plans.
///
/// A registry revision change is the only invalidation signal. Missing material
/// IDs are cached separately, while fallback-only plans are intentionally not
/// retained because their colors can be generated dynamically by scene content.
#[derive(Default)]
pub(super) struct ResolvedLitMaterialPlanCache {
    revision: Option<u64>,
    plans: FxHashMap<u64, OwnedLitMaterialPlan>,
    unresolved: FxHashSet<u64>,
    hits: u64,
    negative_hits: u64,
    misses: u64,
    invalidations: u64,
}

impl ResolvedLitMaterialPlanCache {
    pub(super) fn resolve(
        &mut self,
        registry: &dyn MaterialRegistryApi,
        material_ref: Option<MaterialRef>,
        fallback_color: [f32; 4],
    ) -> OwnedLitMaterialPlan {
        self.synchronize_revision(registry.revision());

        let Some(material_ref) = material_ref else {
            return OwnedLitMaterialPlan::fallback(fallback_color);
        };
        let key = material_ref.id.raw();

        if let Some(plan) = self.plans.get(&key) {
            self.hits = self.hits.saturating_add(1);
            return plan.clone();
        }
        if self.unresolved.contains(&key) {
            self.negative_hits = self.negative_hits.saturating_add(1);
            return OwnedLitMaterialPlan::fallback(fallback_color);
        }

        self.misses = self.misses.saturating_add(1);
        let Some(resolved) = registry.resolve(material_ref.id) else {
            self.unresolved.insert(key);
            return OwnedLitMaterialPlan::fallback(fallback_color);
        };
        let plan = OwnedLitMaterialPlan::from_resolved(&resolved);
        self.plans.insert(key, plan.clone());
        plan
    }

    #[inline]
    pub(super) fn stats(&self) -> ResolvedLitMaterialPlanCacheStats {
        ResolvedLitMaterialPlanCacheStats {
            entries: self.plans.len(),
            unresolved_entries: self.unresolved.len(),
            hits: self.hits,
            negative_hits: self.negative_hits,
            misses: self.misses,
            invalidations: self.invalidations,
        }
    }

    fn synchronize_revision(&mut self, revision: u64) {
        if self.revision == Some(revision) {
            return;
        }
        if self.revision.is_some() {
            self.invalidations = self.invalidations.saturating_add(1);
        }
        self.revision = Some(revision);
        self.plans.clear();
        self.unresolved.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_materials::{MaterialDescriptor, MaterialRegistry};

    #[test]
    fn owned_plan_detaches_texture_paths_from_source_lifetime() {
        let base = String::from("textures/base");
        let normal = String::from("textures/normal");
        let roughness = String::from("textures/roughness");
        let borrowed = LitMaterialPlan {
            base_color: [0.1, 0.2, 0.3, 1.0],
            emissive_radiance: [0.4, 0.5, 0.6],
            alpha_cutoff: 0.25,
            uv_transform: [2.0, 3.0, 0.1, 0.2],
            material_params: [1.0, 0.7, 0.2, 0.9],
            base_color_texture: Some(&base),
            normal_texture: Some(&normal),
            roughness_texture: Some(&roughness),
            double_sided: true,
            alpha_blend: false,
            cast_shadows: true,
            receive_shadows: true,
        };

        let owned = OwnedLitMaterialPlan::from_borrowed(borrowed);
        drop((base, normal, roughness));
        let restored = owned.as_borrowed();

        assert_eq!(restored.base_color_texture, Some("textures/base"));
        assert_eq!(restored.normal_texture, Some("textures/normal"));
        assert_eq!(restored.roughness_texture, Some("textures/roughness"));
        assert_eq!(restored.material_params, [1.0, 0.7, 0.2, 0.9]);
    }

    #[test]
    fn registry_revision_invalidates_owned_plan_cache() {
        let registry = MaterialRegistry::new();
        let material_id = registry.register_named(
            "cache.test",
            MaterialDescriptor {
                base_color: [0.2, 0.3, 0.4, 1.0],
                ..MaterialDescriptor::default()
            },
        );
        let material_ref = Some(MaterialRef { id: material_id });
        let mut cache = ResolvedLitMaterialPlanCache::default();

        let first = cache.resolve(&registry, material_ref, [1.0; 4]);
        assert_eq!(first.as_borrowed().base_color, [0.2, 0.3, 0.4, 1.0]);
        assert_eq!(cache.stats().misses, 1);

        let second = cache.resolve(&registry, material_ref, [1.0; 4]);
        assert_eq!(second.as_borrowed().base_color, [0.2, 0.3, 0.4, 1.0]);
        assert_eq!(cache.stats().hits, 1);

        registry.upsert_named(
            "cache.test",
            MaterialDescriptor {
                base_color: [0.8, 0.7, 0.6, 1.0],
                ..MaterialDescriptor::default()
            },
        );
        let updated = cache.resolve(&registry, material_ref, [1.0; 4]);
        let stats = cache.stats();

        assert_eq!(updated.as_borrowed().base_color, [0.8, 0.7, 0.6, 1.0]);
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.invalidations, 1);
    }
}
