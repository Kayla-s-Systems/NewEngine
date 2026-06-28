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
        let mut registry = RenderDrawListProviderRegistry::from_runtime_providers(
            controller
                .features
                .draw_list_providers
                .runtime_provider_arcs(),
        );
        if let Some(snapshot) = plugin_snapshot {
            registry.sync_plugin_capabilities(snapshot);
        }
        if trace_frame {
            newengine_ulog_api::ulog::debug!(
                "render draw-list providers: {}",
                registry.labels().join(",")
            );
        }

        let providers = registry.providers();
        let visibility = extraction.visibility();
        let mut draw_lists =
            RuntimeDrawListSet::extract(visibility, extraction, providers.as_slice());
        registry.add_external_draw_lists(visibility, &mut draw_lists);

        let mut profile = TimedBreakdown::new();
        {
            let mut build_ctx = DrawListBuildCtx::new(controller, render, &draw_lists);
            profile.time("pass_state", || {
                draw_lists.record_pass_state(extraction, &mut build_ctx)
            })?;

            for provider in providers.iter().copied() {
                profile.time(provider.id(), || {
                    provider.extract(extraction, &mut build_ctx)
                })?;
            }
        }

        drop(providers);
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
