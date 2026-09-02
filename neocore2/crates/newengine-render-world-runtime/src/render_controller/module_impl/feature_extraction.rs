#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::RenderApi;
use newengine_core::EngineResult;
use newengine_plugin_host::PluginsSnapshot;
use newengine_render_feature_api::SceneExtractionCtx;
use newengine_render_frame_graph::{DrawListDesc, DrawListRouteValidationReport, RenderFramePlan};

use super::draw_lists::{DrawListBuildCtx, RenderDrawListProviderRegistry, RuntimeDrawListSet};
use super::profiling::TimedBreakdown;
use crate::render_controller::RuntimeRenderController;

/// Declarative outcome of render feature extraction for a single frame.
///
/// Runtime feature providers, plugin-provided draw-list declarations and the
/// resulting draw-list set are kept together so the frame orchestrator can stay
/// focused on ordering and error policy instead of provider bookkeeping.
pub(super) struct FeatureExtractionFrame {
    registry: RenderDrawListProviderRegistry,
    draw_lists: RuntimeDrawListSet,
    draw_list_descs: Vec<DrawListDesc>,
    profile: TimedBreakdown,
}

impl FeatureExtractionFrame {
    pub(super) fn extract_runtime(
        controller: &mut RuntimeRenderController,
        render: &mut dyn RenderApi,
        extraction: &SceneExtractionCtx<'_>,
        plugin_snapshot: Option<&PluginsSnapshot>,
        trace_frame: bool,
    ) -> EngineResult<Self> {
        let mut profile = TimedBreakdown::new();
        profile.time("plugin_sync", || -> EngineResult<()> {
            if let Some(snapshot) = plugin_snapshot {
                controller
                    .features
                    .draw_list_providers
                    .sync_plugin_capabilities(snapshot);
            }
            Ok(())
        })?;
        let registry = profile.time("registry_snapshot", || -> EngineResult<_> {
            Ok(controller.features.draw_list_providers.frame_snapshot())
        })?;
        if trace_frame {
            newengine_ulog_api::ulog::debug!(
                "render draw-list providers: {}",
                registry.labels().join(",")
            );
        }

        let providers = profile.time("provider_list", || -> EngineResult<_> {
            Ok(registry.providers())
        })?;
        let visibility = extraction.visibility();
        let draw_lists = profile.time("draw_list_set", || -> EngineResult<_> {
            let mut lists =
                RuntimeDrawListSet::extract(visibility, extraction, providers.as_slice());
            registry.add_external_draw_lists(visibility, &mut lists);
            Ok(lists)
        })?;
        {
            let mut build_ctx = DrawListBuildCtx::new(controller, render, &draw_lists);
            profile.time("pass_state", || {
                draw_lists.record_pass_state(extraction, &mut build_ctx)
            })?;

            for provider in providers.iter() {
                profile.time(provider.id(), || {
                    provider.extract(extraction, &mut build_ctx)
                })?;
                let primitive = build_ctx.take_primitive_stage_profile();
                if primitive.sampled {
                    profile.push_measurement(
                        "primitive.directional_shadow",
                        primitive.directional_shadow_ms,
                    );
                    profile.push_measurement("primitive.local_shadow", primitive.local_shadow_ms);
                    profile.push_measurement("primitive.gbuffer", primitive.gbuffer_ms);
                    profile.push_measurement("primitive.forward", primitive.forward_ms);
                }
            }
        }

        drop(providers);
        if extraction.runtime
            && controller.frame.frame_index.is_multiple_of(30)
            && newengine_runtime_policy::render_runtime_policy().primitive_stage_log
        {
            newengine_ulog_api::ulog::info!(
                "render.feature.providers.profile: frame={} total_ms={:.3} {}",
                controller.frame.frame_index,
                profile.total_ms(),
                profile.breakdown(),
            );
        }
        let draw_list_descs = draw_lists.descriptors();
        Ok(Self {
            registry,
            draw_lists,
            draw_list_descs,
            profile,
        })
    }

    #[inline]
    pub(super) fn draw_lists(&self) -> &RuntimeDrawListSet {
        &self.draw_lists
    }

    #[inline]
    pub(super) fn draw_list_descs(&self) -> &[DrawListDesc] {
        &self.draw_list_descs
    }

    #[inline]
    pub(super) fn profile_total_ms(&self) -> f32 {
        self.profile.total_ms()
    }

    #[inline]
    pub(super) fn profile_breakdown(&self) -> String {
        self.profile.breakdown()
    }

    #[inline]
    pub(super) fn validate_routes(
        &self,
        report: &DrawListRouteValidationReport,
    ) -> EngineResult<()> {
        self.registry.validate_routes(report)
    }

    #[inline]
    pub(super) fn extract_external_providers(
        &self,
        extraction: &SceneExtractionCtx<'_>,
        frame_plan: &RenderFramePlan,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        self.registry
            .extract_external_providers(extraction, &self.draw_lists, frame_plan, out)
    }
}
