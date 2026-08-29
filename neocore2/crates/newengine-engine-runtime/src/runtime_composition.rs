#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;
use std::sync::Arc;

use newengine_material_domain_api::MaterialGpuPipelineKey;
use newengine_render_feature_api::RenderFeatureContribution;

use crate::{RuntimeRenderController, WorldRuntimeProvider};

/// Instance-local handoff between selected runtime-unit materializers and the reusable
/// render-controller composition root. Runtime units populate this registry; product profiles
/// only consume it and never name the concrete world/render providers selected by the solver.
#[derive(Default)]
pub struct RuntimeRenderContributionRegistry {
    render_feature_ids: BTreeSet<&'static str>,
    render_features: Vec<RenderFeatureContribution>,
    world_runtime_providers: Vec<Arc<dyn WorldRuntimeProvider>>,
    primary_lit_material_domain: Option<MaterialGpuPipelineKey>,
}

impl RuntimeRenderContributionRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_render_feature(
        &mut self,
        contribution: RenderFeatureContribution,
    ) -> Result<(), String> {
        if contribution.id.trim().is_empty() {
            return Err("render feature contribution id must not be empty".to_owned());
        }
        if !self.render_feature_ids.insert(contribution.id) {
            return Err(format!(
                "render feature contribution '{}' was materialized more than once",
                contribution.id
            ));
        }
        if let Some(key) = contribution.primary_lit_material_domain {
            if let Some(existing) = self.primary_lit_material_domain {
                if existing != key {
                    return Err(format!(
                        "selected render features disagree on primary lit material domain existing='{}' incoming='{}' contribution='{}'",
                        existing.as_str(),
                        key.as_str(),
                        contribution.id,
                    ));
                }
            } else {
                self.primary_lit_material_domain = Some(key);
            }
        }
        self.render_features.push(contribution);
        Ok(())
    }

    pub fn register_world_runtime_provider(&mut self, provider: Arc<dyn WorldRuntimeProvider>) {
        if let Some(existing) = self
            .world_runtime_providers
            .iter_mut()
            .find(|existing| existing.id() == provider.id())
        {
            *existing = provider;
        } else {
            self.world_runtime_providers.push(provider);
        }
    }

    #[inline]
    pub fn render_feature_count(&self) -> usize {
        self.render_features.len()
    }

    #[inline]
    pub fn world_runtime_provider_count(&self) -> usize {
        self.world_runtime_providers.len()
    }

    pub fn apply_to(self, mut controller: RuntimeRenderController) -> RuntimeRenderController {
        for provider in self.world_runtime_providers {
            controller = controller.with_world_runtime_provider(provider);
        }
        for contribution in self.render_features {
            for provider in contribution.draw_list_providers {
                controller = controller.with_draw_list_provider(provider);
            }
            for provider in contribution.light_extraction_providers {
                controller = controller.with_light_extraction_provider(provider);
            }
            if let Some(provider) = contribution.material_pipeline_provider {
                controller = controller.with_material_pipeline_provider(provider);
            }
        }
        if let Some(key) = self.primary_lit_material_domain {
            controller = controller.with_primary_lit_material_domain(key);
        }
        controller
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_render_feature_api::RenderFeatureContribution;

    #[test]
    fn duplicate_render_feature_contribution_is_rejected() {
        let mut registry = RuntimeRenderContributionRegistry::new();
        registry
            .register_render_feature(RenderFeatureContribution::new("test.render.feature"))
            .unwrap();
        assert!(registry
            .register_render_feature(RenderFeatureContribution::new("test.render.feature"))
            .is_err());
    }
}
