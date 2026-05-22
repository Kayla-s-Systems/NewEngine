#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::render::{
    Extent2D, RectI32, RenderApi, RenderFrameDebugSnapshot, RenderTargetId, Viewport,
};
use crate::gameplay::GameRunMode;
use newengine_core::EngineResult;
use newengine_render_feature_api::SceneExtractionCtx;
use newengine_render_frame_graph::{standard_runtime_frame, StandardRuntimePipelineDesc};
use newengine_scene::Scene;
use newengine_ui::draw::UiDrawList;

use super::draw_lists::DrawListBuildCtx;
use super::feature_extraction::FeatureExtractionFrame;
use super::frame_envelope_builder::build_runtime_frame_envelope;
use super::frame_submit::submit_frame_envelope;
use super::profiling::{emit_timed_profile, FrameCpuProfile};
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, WorldFrameState};
use super::{lights, passes, picking, postfx, scene, shadows};
use crate::scene_bridge::{apply_engine_view_postfx, EngineViewTransitionPhase};
use super::super::controller::RuntimeRenderController;

pub(super) struct RenderFrameOrchestrator;

impl RenderFrameOrchestrator {
    pub(super) fn submit_scene_viewport_frame(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        scene: &Scene,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<&UiDrawList>,
        _requested_play_mode: GameRunMode,
        rt: Option<RenderTargetId>,
        scope: RenderFrameScope,
        world_frame: &WorldFrameState,
    ) -> EngineResult<PlayableFrameOutcome> {
        let mut cpu_profile = FrameCpuProfile::new();

        let view_frame = &world_frame.view_frame;
        let view = view_frame.view;
        let viewproj = view.view_projection;
        passes::publish_camera_spawn(&controller.bridges.viewport, view.position_ws, view.forward_ws);
        controller.bridges.viewport.publish_view_frame(
            view.view,
            view.projection,
            scope.vp_w,
            scope.vp_h,
        );
        picking::handle_picking(controller, scene, viewproj, scope.vp_w, scope.vp_h);
        cpu_profile.mark("view");

        let bounds = scene::scene_bounds(scene).unwrap_or_else(scene::default_bounds);
        let lit = match controller.gpu.require_primary_lit_pipeline(r) {
            Ok(lit) => lit,
            Err(e) => {
                Self::end_viewport_after_pipeline_failure(controller, r, ui.cloned(), scope, e)?;
                return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
            }
        };
        cpu_profile.mark("pipeline");

        if let Err(e) = controller.pump_scene_gpu_residency(r, scene) {
            log::warn!("render residency: terrain gpu upload budget failed: {}", e);
        }
        cpu_profile.mark("gpu_residency");

        let camera_position = [view.position_ws.x, view.position_ws.y, view.position_ws.z];
        let base_lights = lights::collect_lights(scene.world()).with_camera_position(camera_position);
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let runtime_profile = controller.runtime_profile().clone();
        let legacy_safe_profile = runtime_profile.legacy_safe_enabled();
        if legacy_safe_profile {
            log_legacy_safe_profile_once();
        }
        let shadow_plan = if !runtime_profile.shadows_enabled() {
            shadows::LightShadowPlan::disabled(lit.white_texture)
        } else {
            match shadows::build_light_shadow_plan(
                controller,
                r,
                scene,
                bounds,
                lit,
                viewproj,
                camera_position,
                [view.forward_ws.x, view.forward_ws.y, view.forward_ws.z],
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
            }
        };

        let render_shadow_map = controller.should_render_shadow_map_this_frame(shadow_plan);
        controller.set_shadow_caster_cull(if render_shadow_map { shadow_plan.caster_cull } else { None });
        Self::trace_shadow_plan(controller, scope.trace_frame, shadow_plan, render_shadow_map);
        cpu_profile.mark("shadow_plan");

        let shadow_frame = if shadow_plan.is_active()
            && !render_shadow_map
            && !controller.shadows.cache_valid
        {
            if scope.trace_frame {
                log::debug!(
                    "render shadow cache: using unshadowed fallback until first shadow map is rendered frame={} target={:?}",
                    controller.frame.frame_index,
                    shadow_plan.render_target()
                );
            }
            shadows::ShadowFrame::disabled(lit.white_texture)
        } else if shadow_plan.is_active() && !render_shadow_map {
            // The cached shadow texture was rendered with the cached light MVP.
            // Keep sampling with that same frame until the next scheduled shadow
            // refresh; otherwise a moving sun would sample an old shadow map with
            // a new light matrix and produce swimming/self-shadowing artefacts.
            controller.cached_shadow_frame().unwrap_or(shadow_plan.frame)
        } else {
            shadow_plan.frame
        };
        let world_lights = base_lights.with_shadow_frame(shadow_frame);
        let extraction = SceneExtractionCtx {
            scene,
            lit,
            viewproj: viewproj,
            camera_position: view.position_ws,
            camera_forward: view.forward_ws,
            bounds,
            lights: world_lights,
            shadow_plan,
            shadow_frame,
            render_shadow_map,
            viewport_extent: extent,
            surface_extent: Extent2D::new(scope.w, scope.h),
            runtime: view_frame.effective_play_mode.is_runtime(),
            debug_overlays: false,
            ui,
        };

        let features = match FeatureExtractionFrame::extract_runtime(
            controller,
            r,
            &extraction,
            plugin_snapshot,
            scope.trace_frame,
        ) {
            Ok(features) => features,
            Err(e) => {
                controller.disable_viewport_pass("draw_list.provider_extraction", &e);
                Self::end_viewport_after_draw_failure(controller, r, ui.cloned(), scope)?;
                return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
            }
        };
        Self::trace_feature_extract_profile(
            controller.frame.frame_index,
            scope.trace_frame,
            features.profile_total_ms(),
            &features.profile_breakdown(),
            ui,
        );
        cpu_profile.mark("feature_extract");

        let shadow_rt_for_graph = if render_shadow_map {
            shadow_plan.render_target()
        } else {
            None
        };
        let draw_list_descs = features.draw_list_descs().to_vec();
        let ui_backdrop = controller.menu.pause.ui_backdrop_postfx();
        let frame_plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(controller.frame.frame_index, Extent2D::new(scope.w, scope.h), extent)
                .viewport_is_surface(scope.direct_surface_viewport)
                .viewport_render_target(rt)
                .shadow(runtime_profile.shadows_enabled() && render_shadow_map && shadow_rt_for_graph.is_some(), shadow_plan.resolution)
                .shadow_cascades(if runtime_profile.shadows_enabled() { shadow_plan.cascade_count() } else { 0 })
                .shadow_render_target(shadow_rt_for_graph)
                .deferred(runtime_profile.deferred_enabled())
                .hdr_scene(runtime_profile.hdr_scene_enabled())
                .postfx(runtime_profile.postfx_enabled())
                .ui(scope.ui_enabled)
                .ui_backdrop_blur(scope.ui_enabled && ui_backdrop.enabled && ui_backdrop.blur_radius_px > 0.05)
                .debug_overlay(false)
                .draw_lists(draw_list_descs.clone()),
        );

        features.validate_routes(&frame_plan.validate_draw_list_routes())?;
        {
            let mut build_ctx = DrawListBuildCtx::new(controller, r, features.draw_lists());
            features.extract_external_providers(&extraction, &frame_plan, &mut build_ctx)?;
        }
        cpu_profile.mark("frame_plan_external");

        let mut postfx = apply_engine_view_postfx(
            postfx::game_sun_postfx_params(scene.world(), viewproj, view.position_ws),
            view_frame.postfx,
        );
        postfx.ui_backdrop = ui_backdrop;
        let frame_envelope = build_runtime_frame_envelope(
            controller.frame.frame_index,
            controller.viewport.clear_color,
            Extent2D::new(scope.w, scope.h),
            extent,
            scope.direct_surface_viewport,
            &frame_plan,
            &draw_list_descs,
            postfx,
            scope.trace_frame,
        );
        cpu_profile.mark("envelope");

        let submit_report = match submit_frame_envelope(r, frame_envelope, scope.trace_frame) {
            Ok(report) => report,
            Err(e) => {
                controller.disable_viewport_pass("render_graph.submit_frame", &e);
                log::error!(
                    "render controller: frame graph submit failed; viewport pass disabled and renderer continues in degraded UI/safe-present mode: {}",
                    e
                );
                let _ = r.discard_recorded_commands();
                let _ = r.end_frame();
                return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
            }
        };
        cpu_profile.mark("submit");
        Self::trace_cpu_profile(controller.frame.frame_index, scope.trace_frame, &cpu_profile);
        if render_shadow_map {
            controller.mark_shadow_map_rendered(shadow_plan);
        }
        controller.diagnostics.overlay_metrics.record_graph_submit(submit_report.clone());

        let mut debug_notes = Vec::new();
        if let Some(report) = view_frame.diagnostics.clone() {
            controller.diagnostics.overlay_metrics.record_view_report(report.clone());
            debug_notes.push(format!(
                "view director={} mode={} view={} dominant={:?} rendered={} input={} lock={} gate_blocked={} blend_active={} blend_alpha={:.3} events={}",
                report.active_director,
                report.active_mode,
                report.active_view_mode,
                report.dominant_director,
                report.rendered_director_count,
                report.input_context,
                report.director_lock_input,
                report.gate_blocked,
                report.frame_blend_active,
                report.frame_blend_alpha,
                report.pending_event_count,
            ));
            if report.transition.phase != EngineViewTransitionPhase::Idle {
                debug_notes.push(format!(
                    "view transition {:?} {:.2}s target={:?}",
                    report.transition.phase,
                    report.transition.elapsed_sec,
                    report.target_entity,
                ));
            }
        }

        Ok(PlayableFrameOutcome::Continue {
            frame_debug_snapshot: Some(RenderFrameDebugSnapshot {
                frame_index: controller.frame.frame_index,
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

    fn trace_feature_extract_profile(
        frame_index: u64,
        trace_frame: bool,
        feature_ms: f32,
        breakdown: &str,
        ui: Option<&UiDrawList>,
    ) {
        let ui_stats = ui.map(Self::ui_draw_list_stats).unwrap_or_else(|| "ui=none".to_owned());
        emit_timed_profile(
            "render feature profile",
            frame_index,
            trace_frame,
            feature_ms,
            breakdown,
            ui_stats,
        );
    }

    fn ui_draw_list_stats(ui: &UiDrawList) -> String {
        let tex_set_bytes: usize = ui
            .texture_delta
            .set
            .values()
            .map(|texture| texture.rgba8.len())
            .sum();
        let patch_bytes: usize = ui
            .texture_delta
            .patches
            .iter()
            .map(|patch| patch.rgba8.len())
            .sum();
        format!(
            "ui(vertices={} indices={} cmds={} tex_set={} tex_set_bytes={} patches={} patch_bytes={} free={})",
            ui.mesh.vertices.len(),
            ui.mesh.indices.len(),
            ui.mesh.cmds.len(),
            ui.texture_delta.set.len(),
            tex_set_bytes,
            ui.texture_delta.patches.len(),
            patch_bytes,
            ui.texture_delta.free.len(),
        )
    }

    fn trace_cpu_profile(
        frame_index: u64,
        trace_frame: bool,
        profile: &FrameCpuProfile,
    ) {
        emit_timed_profile(
            "render cpu profile",
            frame_index,
            trace_frame,
            profile.total_ms(),
            profile.breakdown(),
            "",
        );
    }

    fn end_viewport_after_pipeline_failure(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        controller.disable_viewport_pass("material_gpu_registry.require_primary_lit_pipeline", &error);
        r.set_viewport(Viewport::full(Extent2D::new(scope.w, scope.h)))?;
        r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
        controller.gc_per_draw_ubos(r);
        controller.gc_deferred_rts(r);
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} after viewport disable",
                controller.frame.frame_index
            ));
        }
        r.end_frame()
    }

    fn end_viewport_after_draw_failure(
        controller: &mut RuntimeRenderController,
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
                controller.frame.frame_index
            ));
        }
        r.end_frame()
    }

    fn trace_shadow_plan(
        controller: &RuntimeRenderController,
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
            controller.shadows.cache_valid,
            shadow_plan.render_target(),
            shadow_plan.resolution
        );
    }
}


static LEGACY_SAFE_PROFILE_LOGGED: AtomicBool = AtomicBool::new(false);

fn log_legacy_safe_profile_once() {
    if LEGACY_SAFE_PROFILE_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render controller: legacy GPU safe profile active; preserving original renderer path for capable GPUs, but disabling risky shadows/HDR/postfx/deferred graph branches on this device"
        );
        newengine_core::crash::record_breadcrumb(
            "render controller: legacy GPU safe profile active".to_owned(),
        );
    }
}
