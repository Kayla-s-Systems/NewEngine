#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::render::{
    Extent2D, RectI32, RenderApi, RenderFrameDebugSnapshot, RenderTargetId, TextureFormat, Viewport,
};
use crate::gameplay::GameRunMode;
use newengine_core::{EngineResult, JobSystemHandle};
use newengine_render_feature_api::SceneExtractionCtx;
use newengine_render_frame_graph::{standard_runtime_frame, StandardRuntimePipelineDesc};
use newengine_scene::Scene;
use newengine_ui_api::UiDrawList;

use super::draw_lists::DrawListBuildCtx;
use super::feature_extraction::FeatureExtractionFrame;
use super::frame_envelope_builder::build_runtime_frame_envelope;
use super::frame_snapshots::SceneRenderSnapshot;
use super::frame_submit::submit_frame_envelope;
use super::profiling::{emit_timed_profile, FrameCpuProfile};
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, WorldFrameState};
use super::{lights, passes, picking, postfx, shadows};
use crate::scene_bridge::{apply_engine_view_postfx, EngineViewTransitionPhase};
use super::super::controller::RuntimeRenderController;
use super::super::error_policy::{is_backend_device_lost_error, is_transient_shader_pipeline_error};

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
        job_system: Option<&JobSystemHandle>,
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

        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::SCENE_RENDER_SNAPSHOT,
            newengine_jobs_api::EngineTaskPhase::Scheduled,
            "SceneRenderSnapshot scheduled",
            Self::render_prep_executor_detail(job_system, "SceneRenderSnapshot still borrows Scene; capture is a visible render-prep barrier until the scene read model is Send + 'static."),
            Some(0.0),
        );
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::SCENE_RENDER_SNAPSHOT,
            newengine_jobs_api::EngineTaskPhase::Running,
            "SceneRenderSnapshot running",
            "Capturing DTO-like render read model before feature extraction.",
            None,
        );
        let snapshot = SceneRenderSnapshot::capture(
            controller.frame.frame_index,
            scene,
            viewproj,
            view.position_ws,
            view.forward_ws,
            Extent2D::new(scope.vp_w, scope.vp_h),
            Extent2D::new(scope.w, scope.h),
            ui.is_some(),
            plugin_snapshot.is_some(),
        );
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::SCENE_RENDER_SNAPSHOT,
            newengine_jobs_api::EngineTaskPhase::Completed,
            "SceneRenderSnapshot captured",
            snapshot.diagnostic_detail(),
            Some(1.0),
        );
        let bounds = snapshot.bounds;
        let runtime_profile = controller.runtime_profile().clone();
        let scene_color_format = if runtime_profile.hdr_scene_enabled() {
            super::super::render_quality::SCENE_HDR_COLOR_FORMAT
        } else {
            TextureFormat::Bgra8Unorm
        };
        let lit = match controller.gpu.require_primary_lit_pipeline_for(scene_color_format, r) {
            Ok(lit) => lit,
            Err(e) if is_transient_shader_pipeline_error(&e) => {
                Self::end_viewport_after_transient_pipeline_wait(
                    controller,
                    r,
                    ui.cloned(),
                    scope,
                    e,
                )?;
                return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
            }
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

        let camera_position = [snapshot.camera_position.x, snapshot.camera_position.y, snapshot.camera_position.z];
        let base_lights = lights::collect_lights(scene.world()).with_camera_position(camera_position);
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let gpu_safe_profile = runtime_profile.gpu_safe_enabled();
        if gpu_safe_profile {
            log_gpu_safe_profile_once();
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
                [snapshot.camera_forward.x, snapshot.camera_forward.y, snapshot.camera_forward.z],
                extent,
                snapshot.surface_extent,
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
            viewport_extent: snapshot.viewport_extent,
            surface_extent: snapshot.surface_extent,
            runtime: view_frame.effective_play_mode.is_runtime(),
            debug_overlays: false,
            ui,
        };

        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::FEATURE_EXTRACT,
            newengine_jobs_api::EngineTaskPhase::Scheduled,
            "RenderPrep pass scheduled",
            Self::render_prep_executor_detail(job_system, "Feature extraction is the profiler hotspot. Provider-safe DTO building should move to engine.jobs; RenderApi command recording stays on the render thread."),
            Some(0.0),
        );
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::FEATURE_EXTRACT,
            newengine_jobs_api::EngineTaskPhase::Running,
            "RenderPrep pass running",
            "Feature extraction is executing on the render-thread barrier because current providers still record RenderApi command lists. Treat this as the synchronous fallback path, not the target architecture.",
            None,
        );
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
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::FEATURE_EXTRACT,
            newengine_jobs_api::EngineTaskPhase::Completed,
            "RenderPrep pass completed",
            format!("Feature extraction completed profile_ms={:.2} breakdown={}", features.profile_total_ms(), features.profile_breakdown()),
            Some(1.0),
        );
        cpu_profile.mark("feature_extract");

        let shadow_rt_for_graph = if render_shadow_map {
            shadow_plan.render_target()
        } else {
            None
        };
        let draw_list_descs = features.draw_list_descs().to_vec();
        let ui_backdrop = controller.ui.primary.ui_backdrop_postfx();
        let ui_enabled = scope.ui_enabled || ui.is_some();
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
                .ui(ui_enabled)
                .ui_backdrop_blur(ui_enabled && ui_backdrop.enabled && ui_backdrop.blur_radius_px > 0.05)
                .debug_overlay(false)
                .draw_lists(draw_list_descs.clone()),
        );

        features.validate_routes(&frame_plan.validate_draw_list_routes())?;
        {
            let mut build_ctx = DrawListBuildCtx::new(controller, r, features.draw_lists());
            features.extract_external_providers(&extraction, &frame_plan, &mut build_ctx)?;
        }
        if let Some(ui_draw_list) = ui {
            // Stage the provider-owned UI packet directly at the renderer boundary as well as
            // through the draw-list route. This keeps modal UI visible even when the active
            // frame profile temporarily has no Ui draw-list provider, or when a graph compile
            // path skips the UI composite pass while the cursor/focus policy already switched
            // to modal mode. The call stays provider-neutral: it targets RenderApi, not Vulkan,
            // a concrete UI provider, or any other backend implementation.
            r.set_ui_draw_list(ui_draw_list.clone());
        }
        cpu_profile.mark("frame_plan_external");

        let mut postfx = apply_engine_view_postfx(
            postfx::game_sun_postfx_params(scene.world(), viewproj, view.position_ws),
            view_frame.postfx,
        );
        postfx.ui_backdrop = ui_backdrop;
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::FRAME_ENVELOPE,
            newengine_jobs_api::EngineTaskPhase::Scheduled,
            "FrameEnvelope staging scheduled",
            "FrameEnvelope packet staging is the render-thread handoff boundary: RenderPrep produces packets, RenderApi recording consumes only the envelope.",
            Some(0.0),
        );
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
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::FRAME_ENVELOPE,
            newengine_jobs_api::EngineTaskPhase::Completed,
            "FrameEnvelope packet staged",
            "RenderApi submit is now consuming a staged FrameEnvelope instead of constructing world packets inside submit.",
            Some(1.0),
        );
        cpu_profile.mark("envelope");

        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::RENDER_SUBMIT,
            newengine_jobs_api::EngineTaskPhase::Running,
            "Render submit consuming packets",
            "Render submit is consuming the prepared frame envelope. Heavy world construction must happen before this point in RenderPrep/Streaming/AssetIo jobs.",
            None,
        );
        let submit_report = match submit_frame_envelope(r, frame_envelope, scope.trace_frame) {
            Ok(report) => report,
            Err(e) => {
                let message = e.to_string();
                controller.disable_viewport_pass("render_graph.submit_frame", &message);
                log::error!(
                    "render controller: frame graph submit failed; viewport pass disabled and renderer continues in degraded UI/safe-present mode: {}",
                    message
                );
                if is_backend_device_lost_error(&e) {
                    controller.record_render_backend_error("render_graph.submit_frame", e)?;
                } else {
                    let _ = r.discard_recorded_commands();
                    let _ = r.end_frame();
                }
                return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
            }
        };
        cpu_profile.mark("submit");
        Self::publish_render_job_pass_event(
            controller.frame.frame_index,
            newengine_jobs_api::job_pass::RENDER_SUBMIT,
            newengine_jobs_api::EngineTaskPhase::Completed,
            "Render submit completed",
            format!("Frame envelope submitted cpu_record_ms={:.2} gpu_submit_ms={:.2}", submit_report.cpu_record_ms, submit_report.gpu_submit_ms),
            Some(1.0),
        );
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


    fn render_prep_executor_detail(
        job_system: Option<&JobSystemHandle>,
        detail: &'static str,
    ) -> String {
        match job_system {
            Some(jobs) => format!(
                "{detail} engine.jobs available worker_threads={} pending_render_prep={}; target split: jobs build provider-safe packets, render thread submits GPU/backend envelope.",
                jobs.worker_threads(),
                jobs.pending_for_lane(newengine_core::JobLane::RenderPrep),
            ),
            None => format!(
                "{detail} engine.jobs handle unavailable for this frame; render-prep remains a main-thread barrier."
            ),
        }
    }

    fn publish_render_job_pass_event(
        frame_index: u64,
        pass: &'static str,
        phase: newengine_jobs_api::EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) {
        let mut event = newengine_jobs_api::EngineTaskEvent::new(
            format!("render.frame.{frame_index}.{pass}"),
            "render.frame-orchestrator",
            "engine.render",
            "render",
            format!("render:{pass}"),
            "render-prep",
            phase,
            status.into(),
            detail.into(),
        )
        .with_frame_id(frame_index)
        .with_dependency_group(format!("frame.{frame_index}.render"))
        .with_job_domain(newengine_jobs_api::job_domain::ENGINE_RENDER)
        .with_job_pass(pass)
        .with_priority("critical")
        .with_executor("main-thread-barrier")
        .with_controls(false, false);
        if let Some(progress) = progress_01 {
            event = event.with_progress(progress);
        }
        let job_event = newengine_jobs_api::EngineJobEventV1::new(
            event.clone(),
            newengine_jobs_api::JobExecutorKind::MainThreadBarrier,
            "render-frame-job-pass",
        );
        if let Ok(payload) = serde_json::to_vec(&event) {
            let _ = newengine_plugin_host::host_context::publish_event(
                newengine_jobs_api::ENGINE_TASK_EVENT_TOPIC_V1,
                &payload,
            );
        }
        if let Ok(payload) = serde_json::to_vec(&job_event) {
            let _ = newengine_plugin_host::host_context::publish_event(
                newengine_jobs_api::ENGINE_JOB_EVENT_TOPIC_V1,
                &payload,
            );
        }
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

    fn end_viewport_after_transient_pipeline_wait(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        log_transient_pipeline_wait_once(
            controller.frame.frame_index,
            &format!("{}", error),
        );
        let _ = r.discard_recorded_commands();
        r.set_viewport(Viewport::full(Extent2D::new(scope.w, scope.h)))?;
        r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
        controller.gc_per_draw_ubos(r);
        controller.gc_deferred_rts(r);
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} while material shader pipeline is pending",
                controller.frame.frame_index
            ));
        }
        r.end_frame()
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


static GPU_SAFE_PROFILE_LOGGED: AtomicBool = AtomicBool::new(false);

fn log_gpu_safe_profile_once() {
    if GPU_SAFE_PROFILE_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render controller: legacy conservative GPU profile active; high-cost feature branches are disabled only by explicit runtime profile policy"
        );
        newengine_core::crash::record_breadcrumb(
            "render controller: legacy conservative GPU profile active".to_owned(),
        );
    }
}

static TRANSIENT_SHADER_PIPELINE_WAIT_LOGGED: AtomicBool = AtomicBool::new(false);

fn log_transient_pipeline_wait_once(frame_index: u64, error: &str) {
    if TRANSIENT_SHADER_PIPELINE_WAIT_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render controller: material pipeline not ready yet; shader compile remains async and viewport will retry next frame frame={} err='{}'",
            frame_index,
            error
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: transient material shader pipeline wait frame={} err='{}'",
            frame_index, error
        ));
    } else {
        log::debug!(
            "render controller: material pipeline still pending; retrying next frame frame={} err='{}'",
            frame_index,
            error
        );
    }
}
