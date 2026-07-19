use super::*;

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
        let hdr_scene_enabled = runtime_profile.hdr_scene_enabled() && !external_preview_target;
        let deferred_enabled = runtime_profile.deferred_enabled() && !external_preview_target;
        let postfx_enabled = runtime_profile.postfx_enabled() && !external_preview_target;
        let shadows_enabled = runtime_profile.shadows_enabled() && !external_preview_target;
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

        if let Err(e) = controller.pump_scene_gpu_residency(r, scene) {
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
        let base_lights =
            lights::collect_lights(scene.world()).with_camera_position(camera_position);
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let gpu_safe_profile = runtime_profile.gpu_safe_enabled();
        if gpu_safe_profile {
            log_gpu_safe_profile_once();
        }
        let shadow_plan = if !shadows_enabled {
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
                [
                    snapshot.camera_forward.x,
                    snapshot.camera_forward.y,
                    snapshot.camera_forward.z,
                ],
                extent,
                snapshot.surface_extent,
                plugin_snapshot,
            ) {
                Ok(plan) => plan,
                Err(e) => {
                    newengine_ulog_api::ulog::warn!(
                        "render controller: shadow plan disabled for this frame: {}",
                        e
                    );
                    let _ = r.discard_recorded_commands();
                    shadows::LightShadowPlan::disabled(lit.white_texture)
                }
            }
        };

        let render_shadow_map = controller.should_render_shadow_map_this_frame(shadow_plan);
        controller.set_shadow_caster_cull(if render_shadow_map {
            shadow_plan.caster_cull
        } else {
            None
        });
        Self::trace_shadow_plan(
            controller,
            scope.trace_frame,
            shadow_plan,
            render_shadow_map,
        );
        cpu_profile.mark("shadow_plan");

        let shadow_frame = if shadow_plan.is_active()
            && !render_shadow_map
            && !controller.shadows.cache_valid
        {
            if scope.trace_frame {
                newengine_ulog_api::ulog::debug!(
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
            controller
                .cached_shadow_frame()
                .unwrap_or(shadow_plan.frame)
        } else {
            shadow_plan.frame
        };
        let world_lights = base_lights.with_shadow_frame(shadow_frame);
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
            deferred: deferred_enabled,
            viewport_extent: snapshot.viewport_extent,
            surface_extent: snapshot.surface_extent,
            runtime: view_frame.effective_play_mode.is_runtime(),
            debug_overlays: false,
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
        if render_shadow_map {
            controller.mark_shadow_map_rendered(shadow_plan);
        }
        controller
            .diagnostics
            .overlay_metrics
            .record_graph_submit(submit_report.clone());

        let mut debug_notes = Vec::new();
        if let Some(report) = view_frame.diagnostics.clone() {
            controller
                .diagnostics
                .overlay_metrics
                .record_view_report(report.clone());
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
                    report.transition.phase, report.transition.elapsed_sec, report.target_entity,
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
}
