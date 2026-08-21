use super::*;

#[path = "submit/shadow_debug.rs"]
mod shadow_debug;
#[path = "submit/shadow_setup.rs"]
mod shadow_setup;
#[path = "submit/finalize.rs"]
mod finalize;

use shadow_debug::shadow_receiver_debug_mode;
use shadow_setup::{prepare_shadow_setup, ShadowSetup};
use finalize::{finalize_successful_submit, SuccessfulSubmit};

impl RenderFrameOrchestrator {
    pub(in super::super) fn submit_scene_viewport_frame(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        scene: &Scene,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<&UiDrawList>,
        _requested_play_mode: GameRunMode,
        rt: Option<RenderTargetId>,
        scope: RenderFrameScope,
        world_frame: &WorldFrameState,
        thread_pool: Option<&ThreadPoolHandle>,
    ) -> EngineResult<PlayableFrameOutcome> {
        let mut cpu_profile = FrameCpuProfile::new();

        let view_frame = &world_frame.view_frame;
        let view = view_frame.view;
        let viewproj = view.view_projection;
        passes::publish_camera_spawn(
            &controller.bridges.viewport,
            view.position_ws,
            view.forward_ws,
        );
        controller.bridges.viewport.publish_view_frame(
            view.view,
            view.projection,
            scope.vp_w,
            scope.vp_h,
        );
        picking::handle_picking(controller, scene, viewproj, scope.vp_w, scope.vp_h);
        cpu_profile.mark("view");

        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::SCENE_RENDER_SNAPSHOT,
            newengine_task_api::EngineTaskPhase::Scheduled,
            "SceneRenderSnapshot scheduled",
            Self::render_prep_executor_detail(thread_pool, "SceneRenderSnapshot still borrows Scene; capture is a visible render-prep barrier until the scene read model is Send + 'static."),
            Some(0.0),
        );
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::SCENE_RENDER_SNAPSHOT,
            newengine_task_api::EngineTaskPhase::Running,
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
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::SCENE_RENDER_SNAPSHOT,
            newengine_task_api::EngineTaskPhase::Completed,
            "SceneRenderSnapshot captured",
            snapshot.diagnostic_detail(),
            Some(1.0),
        );
        let bounds = snapshot.bounds;
        let runtime_profile = controller.runtime_profile().clone();
        let external_preview_target = controller.external_preview_target_active();
        let editor_active = controller.editor_viewport.is_active();
        let editor_shading = editor_active.then(|| controller.editor_viewport.shading());
        let editor_debug_shading = editor_shading
            .is_some_and(|mode| mode != newengine_ui_api::UiEditorViewportShading::Lit);
        let editor_wireframe =
            editor_shading == Some(newengine_ui_api::UiEditorViewportShading::Wireframe);
        let editor_show_overlays = editor_active && {
            let state = controller.editor_viewport.state();
            state.show_grid || state.show_bounds || state.show_collision
        };
        // Editor debug-line overlays use the canonical BGRA viewport pipeline.
        // Keep authoring viewport LDR so grid/bounds/gizmos never bind an HDR-incompatible pipeline.
        let hdr_scene_enabled =
            runtime_profile.hdr_scene_enabled() && !external_preview_target && !editor_active;
        let deferred_enabled =
            runtime_profile.deferred_enabled() && !external_preview_target && !editor_debug_shading;
        let postfx_enabled =
            runtime_profile.postfx_enabled() && !external_preview_target && !editor_debug_shading;
        let shadows_enabled =
            runtime_profile.shadows_enabled() && !external_preview_target && !editor_debug_shading;
        let scene_color_format = if hdr_scene_enabled {
            crate::render_controller::render_quality::SCENE_HDR_COLOR_FORMAT
        } else {
            TextureFormat::Bgra8Unorm
        };
        let lit = match controller.gpu.require_primary_lit_pipeline_for(
            scene_color_format,
            deferred_enabled,
            r,
        ) {
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

        if let Err(e) = controller.pump_scene_gpu_residency(r, scene, thread_pool) {
            newengine_ulog_api::ulog::warn!(
                "render residency: terrain gpu upload budget failed: {}",
                e
            );
        }
        cpu_profile.mark("gpu_residency");

        let camera_position = [
            snapshot.camera_position.x,
            snapshot.camera_position.y,
            snapshot.camera_position.z,
        ];
        let mut base_lights = lights::collect_lights(scene.world())
            .with_camera_position(camera_position)
            .with_camera_forward([
                snapshot.camera_forward.x,
                snapshot.camera_forward.y,
                snapshot.camera_forward.z,
            ])
            .with_shadow_receiver_debug_mode(shadow_receiver_debug_mode());
        if editor_debug_shading {
            // Unlit/Wireframe are editor visualization modes, not alternate world lighting.
            // Keep texture/material color while neutralizing all scene light contribution.
            base_lights.ambient = [1.0, 1.0, 1.0, 1.0];
            base_lights.dir_dir_intensity[3] = 0.0;
            base_lights.point_count_pad[0] = 0.0;
            for point in &mut base_lights.point_color_intensity {
                point[3] = 0.0;
            }
            base_lights.spot_count_pad[0] = 0.0;
            for spot in &mut base_lights.spot_color_intensity {
                spot[3] = 0.0;
            }
        }
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let gpu_safe_profile = runtime_profile.gpu_safe_enabled();
        if gpu_safe_profile {
            log_gpu_safe_profile_once();
        }
        let ShadowSetup {
            shadow_plan,
            render_shadow_map,
            local_shadow_plan,
            render_local_shadow_map,
            shadow_frame,
            local_shadow_frame,
            world_lights,
        } = prepare_shadow_setup(
            controller,
            r,
            scene,
            plugin_snapshot,
            &snapshot,
            world_frame,
            lit,
            base_lights,
            shadows_enabled,
            extent,
            scope.trace_frame,
        );

        let extraction = SceneExtractionCtx {
            scene,
            lit,
            viewproj,
            camera_position: view.position_ws,
            camera_forward: view.forward_ws,
            bounds,
            lights: world_lights,
            shadow_plan,
            shadow_frame,
            render_shadow_map,
            local_shadow_plan,
            local_shadow_frame,
            render_local_shadow_map,
            deferred: deferred_enabled,
            viewport_extent: snapshot.viewport_extent,
            surface_extent: snapshot.surface_extent,
            runtime: view_frame.effective_play_mode.is_runtime(),
            debug_overlays: editor_wireframe || editor_show_overlays,
            ui,
        };

        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::FEATURE_EXTRACT,
            newengine_task_api::EngineTaskPhase::Scheduled,
            "RenderPrep pass scheduled",
            Self::render_prep_executor_detail(thread_pool, "Feature extraction is the profiler hotspot. Provider-safe DTO building should move to engine.threading; RenderApi command recording stays on the render thread."),
            Some(0.0),
        );
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::FEATURE_EXTRACT,
            newengine_task_api::EngineTaskPhase::Running,
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
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::FEATURE_EXTRACT,
            newengine_task_api::EngineTaskPhase::Completed,
            "RenderPrep pass completed",
            format!(
                "Feature extraction completed profile_ms={:.2} breakdown={}",
                features.profile_total_ms(),
                features.profile_breakdown()
            ),
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
            StandardRuntimePipelineDesc::new(
                controller.frame.frame_index,
                Extent2D::new(scope.w, scope.h),
                extent,
            )
            .viewport_is_surface(scope.direct_surface_viewport)
            .viewport_render_target(rt)
            .shadow(
                shadows_enabled && render_shadow_map && shadow_rt_for_graph.is_some(),
                shadow_plan.resolution,
            )
            .shadow_cascades(if shadows_enabled {
                shadow_plan.cascade_count()
            } else {
                0
            })
            .shadow_render_target(shadow_rt_for_graph)
            .local_shadow(
                render_local_shadow_map && local_shadow_plan.render_target().is_some(),
                local_shadow_plan.render_target(),
                local_shadow_frame.atlas_extent,
            )
            .deferred(deferred_enabled)
            .hdr_scene(hdr_scene_enabled)
            .postfx(postfx_enabled)
            .ui(ui_enabled)
            .ui_backdrop_blur(
                ui_enabled && ui_backdrop.enabled && ui_backdrop.blur_radius_px > 0.05,
            )
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
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::FRAME_ENVELOPE,
            newengine_task_api::EngineTaskPhase::Scheduled,
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
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::FRAME_ENVELOPE,
            newengine_task_api::EngineTaskPhase::Completed,
            "FrameEnvelope packet staged",
            "RenderApi submit is now consuming a staged FrameEnvelope instead of constructing world packets inside submit.",
            Some(1.0),
        );
        cpu_profile.mark("envelope");

        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::RENDER_SUBMIT,
            newengine_task_api::EngineTaskPhase::Running,
            "Render submit consuming packets",
            "Render submit is consuming the prepared frame envelope. Heavy world construction must happen before this point in RenderPrep/Streaming/AssetIo jobs.",
            None,
        );
        let submit_report = match submit_frame_envelope(r, frame_envelope, scope.trace_frame) {
            Ok(report) => report,
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
                let message = e.to_string();
                controller.disable_viewport_pass("render_graph.submit_frame", &message);
                newengine_ulog_api::ulog::error!(
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
        Self::publish_render_task_pass_event(
            controller.frame.frame_index,
            newengine_task_api::task_pass::RENDER_SUBMIT,
            newengine_task_api::EngineTaskPhase::Completed,
            "Render submit completed",
            format!(
                "Frame envelope submitted cpu_record_ms={:.2} gpu_submit_ms={:.2}",
                submit_report.cpu_record_ms, submit_report.gpu_submit_ms
            ),
            Some(1.0),
        );
        Self::trace_cpu_profile(
            controller.frame.frame_index,
            scope.trace_frame,
            &cpu_profile,
        );
        Ok(finalize_successful_submit(
            controller,
            SuccessfulSubmit {
                scope,
                view_frame,
                base_lights,
                render_shadow_map,
                shadow_plan,
                render_local_shadow_map,
                local_shadow_plan,
                frame_plan: &frame_plan,
                submit_report,
            },
        ))
    }
}
