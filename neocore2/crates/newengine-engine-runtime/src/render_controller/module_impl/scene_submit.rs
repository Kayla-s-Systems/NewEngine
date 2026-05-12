use newengine_core::render::{
    Extent2D, RectI32, RenderApi, RenderFrameDebugSnapshot, RenderFrameEnvelope, RenderTargetId, Viewport,
};
use crate::gameplay::EditorPlayMode;
use newengine_camera_runtime::CameraManagerResource;
use newengine_core::EngineResult;
use newengine_render_frame_graph::{standard_runtime_frame, StandardRuntimePipelineDesc};
use newengine_scene::Scene;
use newengine_ui::draw::UiDrawList;

use super::draw_lists::{DrawListBuildCtx, RuntimeDrawListSet, SceneExtractionCtx};
use super::frame_submit::submit_frame_envelope;
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, WorldFrameState};
use super::providers::standard_runtime_draw_list_provider_registry;
use super::{lights, passes, picking, scene, shadows};
use super::super::controller::RuntimeRenderController;
use super::super::gpu::ensure_lit_pipeline;

impl RuntimeRenderController {
    pub(super) fn submit_scene_viewport_frame(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<&UiDrawList>,
        requested_play_mode: EditorPlayMode,
        rt: Option<RenderTargetId>,
        scope: RenderFrameScope,
        world_frame: &WorldFrameState,
    ) -> EngineResult<PlayableFrameOutcome> {
        let camera = &world_frame.camera_frame;
        let rig = camera.rig;
        let viewproj = camera.matrices.view_proj;
        passes::publish_camera_spawn(&self.viewport_bridge, &rig);
        self.viewport_bridge.publish_camera_frame(
            camera.matrices.view,
            camera.matrices.proj,
            scope.vp_w,
            scope.vp_h,
        );
        picking::handle_picking(self, scene, viewproj, scope.vp_w, scope.vp_h);

        let bounds = scene::scene_bounds(scene).unwrap_or_else(scene::default_bounds);
        let lit = match ensure_lit_pipeline(&mut self.lit, r) {
            Ok(lit) => lit,
            Err(e) => {
                self.end_viewport_after_pipeline_failure(r, ui.cloned(), scope, e)?;
                return Ok(PlayableFrameOutcome::EndedEarly);
            }
        };

        let camera_position = [rig.position.x, rig.position.y, rig.position.z];
        let base_lights = lights::collect_lights(scene.world()).with_camera_position(camera_position);
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let shadow_plan = match shadows::build_light_shadow_plan(
            self,
            r,
            scene,
            bounds,
            lit,
            viewproj,
            camera_position,
            extent,
            Extent2D::new(scope.w, scope.h),
            plugin_snapshot,
        ) {
            Ok(plan) => plan,
            Err(e) => {
                log::warn!("render controller: shadow plan disabled for this frame: {}", e);
                let _ = r.discard_recorded_commands();
                shadows::LightShadowPlan::disabled(lit.white_texture)
            }
        };

        let render_shadow_map = self.should_render_shadow_map_this_frame(shadow_plan);
        self.trace_shadow_plan(scope.trace_frame, shadow_plan, render_shadow_map);

        let shadow_frame = shadow_plan.frame;
        let world_lights = base_lights.with_shadow(shadow_frame.light_mvp, shadow_frame.params);
        let extraction = SceneExtractionCtx {
            scene,
            lit,
            viewproj: viewproj,
            rig: &rig,
            bounds,
            lights: world_lights,
            shadow_plan,
            shadow_frame,
            render_shadow_map,
            viewport_extent: extent,
            surface_extent: Extent2D::new(scope.w, scope.h),
            runtime: world_frame.effective_play_mode.is_runtime(),
            editor_overlays: !world_frame.effective_play_mode.is_runtime()
                && !requested_play_mode.is_runtime(),
            ui,
        };

        let mut provider_registry = standard_runtime_draw_list_provider_registry();
        if let Some(snapshot) = plugin_snapshot {
            provider_registry.sync_plugin_capabilities(snapshot);
        }
        if scope.trace_frame {
            log::debug!(
                "render draw-list providers: {}",
                provider_registry.labels().join(",")
            );
        }

        let providers = provider_registry.providers();
        let visibility = extraction.visibility();
        let mut draw_lists = RuntimeDrawListSet::extract(visibility, &extraction, providers.as_slice());
        provider_registry.add_external_draw_lists(visibility, &mut draw_lists);

        let provider_result = {
            let mut build_ctx = DrawListBuildCtx::new(self, r, &draw_lists);
            draw_lists.record_pass_state(&extraction, &mut build_ctx).and_then(|()| {
                for provider in providers.iter().copied() {
                    provider.extract(&extraction, &mut build_ctx)?;
                }
                Ok(())
            })
        };
        if let Err(e) = provider_result {
            self.disable_viewport_pass("draw_list.provider_extraction", &e);
            self.end_viewport_after_draw_failure(r, ui.cloned(), scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly);
        }

        let shadow_rt_for_graph = if render_shadow_map {
            shadow_plan.render_target()
        } else {
            None
        };
        let draw_list_descs = draw_lists.descriptors();
        let frame_plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(self.frame_index, Extent2D::new(scope.w, scope.h), extent)
                .viewport_is_surface(scope.direct_surface_viewport)
                .viewport_render_target(rt)
                .shadow(render_shadow_map && shadow_rt_for_graph.is_some(), shadow_plan.resolution)
                .shadow_render_target(shadow_rt_for_graph)
                .deferred(false)
                .postfx(false)
                .ui(scope.ui_enabled)
                .debug_overlay(true)
                .draw_lists(draw_list_descs.clone()),
        );

        provider_registry.validate_routes(&frame_plan.validate_draw_list_routes())?;
        {
            let mut build_ctx = DrawListBuildCtx::new(self, r, &draw_lists);
            provider_registry.extract_external_providers(
                &extraction,
                &draw_lists,
                &frame_plan,
                &mut build_ctx,
            )?;
        }

        if scope.trace_frame {
            let phases = frame_plan
                .phase_order()
                .map(|phase| phase.label())
                .collect::<Vec<_>>()
                .join(" -> ");
            log::debug!("render frame envelope: frame={} phases={}", self.frame_index, phases);
        }
        let frame_envelope = RenderFrameEnvelope::new(
            self.frame_index,
            self.clear_color,
            Extent2D::new(scope.w, scope.h),
            extent,
            scope.direct_surface_viewport,
            frame_plan.graph.clone(),
        )
        .with_draw_lists(draw_list_descs.iter().map(|desc| desc.kind));

        let submit_report = match submit_frame_envelope(r, frame_envelope, scope.trace_frame) {
            Ok(report) => report,
            Err(e) => {
                let _ = r.discard_recorded_commands();
                let _ = r.end_frame();
                return Err(e);
            }
        };
        if render_shadow_map {
            self.mark_shadow_map_rendered();
        }
        self.overlay_metrics.record_graph_submit(submit_report.clone());

        let mut debug_notes = Vec::new();
        if let Some(report) = scene
            .world()
            .resource::<CameraManagerResource>()
            .map(|manager| manager.report())
        {
            self.overlay_metrics.record_camera_report(report);
            debug_notes.push(format!(
                "camera director={:?} mode={:?} input={:?} gate_blocked={} blend_active={} blend_alpha={:.3}",
                report.active_director,
                report.active_mode,
                report.input_context,
                report.gate_blocked,
                report.frame_blend_active,
                report.frame_blend_alpha,
            ));
        }

        Ok(PlayableFrameOutcome::Continue {
            frame_debug_snapshot: Some(RenderFrameDebugSnapshot {
                frame_index: self.frame_index,
                surface_extent: [scope.w, scope.h],
                viewport_extent: [scope.vp_w, scope.vp_h],
                direct_surface_viewport: scope.direct_surface_viewport,
                graph_label: frame_plan
                    .graph
                    .label
                    .clone()
                    .unwrap_or_else(|| "<unnamed>".to_owned()),
                phase_order: frame_plan
                    .phase_order()
                    .map(|phase| phase.label().to_owned())
                    .collect(),
                draw_list_stats: submit_report.draw_list_stats.clone(),
                executed_passes: submit_report.executed_passes,
                skipped_passes: submit_report.skipped_passes,
                cpu_record_ms: submit_report.cpu_record_ms,
                gpu_submit_ms: submit_report.gpu_submit_ms,
                queued_upload_jobs: 0,
                queued_upload_bytes: 0,
                resource_buffers: 0,
                resource_textures: 0,
                resource_pipelines: 0,
                notes: debug_notes,
            }),
        })
    }

    fn end_viewport_after_pipeline_failure(
        &mut self,
        r: &mut dyn RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        self.disable_viewport_pass("ensure_lit_pipeline", &error);
        r.set_viewport(Viewport::full(Extent2D::new(scope.w, scope.h)))?;
        r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
        self.gc_per_draw_ubos(r);
        self.gc_deferred_rts(r);
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} after viewport disable",
                self.frame_index
            ));
        }
        r.end_frame()
    }

    fn end_viewport_after_draw_failure(
        &mut self,
        r: &mut dyn RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        let _ = r.discard_recorded_commands();
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} after draw-list provider failure",
                self.frame_index
            ));
        }
        r.end_frame()
    }

    fn trace_shadow_plan(
        &self,
        trace_frame: bool,
        shadow_plan: shadows::LightShadowPlan,
        render_shadow_map: bool,
    ) {
        if !trace_frame {
            return;
        }
        let shadow_kind = shadow_plan
            .light_kind
            .map(|kind| kind.label())
            .unwrap_or("none");
        log::debug!(
            "render shadow plan: kind={} active={} render_this_frame={} cache_valid={} target={:?} resolution={}",
            shadow_kind,
            shadow_plan.is_active(),
            render_shadow_map,
            self.shadow_cache_valid,
            shadow_plan.render_target(),
            shadow_plan.resolution
        );
    }
}
