#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera_runtime::{
    cursor_state_for_nav, step_camera_nav, BoundsSphere as CamBoundsSphere, CameraNavFrameRequest,
    CameraNavInput, CameraNavParams,
};
use newengine_core::host_events::WindowInitSize;
use newengine_core::host_events::{CursorState, HostEvent, WindowHostEvent};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, Extent2D, RectI32, RenderApi, RenderDrawListKind,
    RenderFrameDebugSnapshot, RenderGraphSubmitReport, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_math::{Quat, Vec2, Vec3};
use newengine_render_frame_graph::{
    standard_runtime_frame, RenderFramePlan, StandardRuntimePipelineDesc,
};
use newengine_ui::draw::UiDrawList;

use super::controller::RuntimeRenderController;
use super::gpu::ensure_lit_pipeline;
use crate::gameplay::{
    apply_player_input, attach_active_camera_to_player, capture_runtime_world_snapshot,
    clear_player_input, detach_active_camera_from_player, first_player, restore_runtime_world_snapshot,
    run_schedule, EditorPlayMode,
};

mod draw_lists;
mod external_contribution_lowering;
mod grid;
mod input;
mod light_extraction;
mod light_providers;
mod lights;
mod passes;
mod passes_ubo;
mod picking;
mod providers;
mod readiness;
mod scene;
mod shadows;

use draw_lists::{DrawListBuildCtx, RuntimeDrawListSet, SceneExtractionCtx};
use providers::standard_runtime_draw_list_provider_registry;
use input::ViewportInputSnap;

#[inline]
fn quat_from_forward_z(dir_ws: Vec3) -> Quat {
    let fwd = Vec3::Z;
    let d = dir_ws.normalize_or_zero();
    if d.length_squared() <= 1e-8 {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(fwd, d)
}

#[inline]
fn submit_frame_plan_v3(
    r: &mut dyn RenderApi,
    plan: &RenderFramePlan,
    trace_frame: bool,
) -> EngineResult<RenderGraphSubmitReport> {
    let report = r.submit_render_graph(plan.graph.clone())?;
    if trace_frame {
        log::debug!(
            "render frame graph v3: submitted graph='{}' passes={} executed_native={} skipped_native={} barriers(raw={}, war={}, waw={})",
            plan.graph.label.as_deref().unwrap_or("<unnamed>"),
            report.compile.pass_count,
            report.executed_passes,
            report.skipped_passes,
            report.compile.barriers.read_after_write,
            report.compile.barriers.write_after_read,
            report.compile.barriers.write_after_write,
        );
        if !report.draw_list_stats.is_empty() {
            let draw_lists = report
                .draw_list_stats
                .iter()
                .map(|it| {
                    format!(
                        "{}: recorded={} draw={} indexed={} skipped={} state(vp={},sc={},pipe={},vb={},ib={},bg={},invalid={})",
                        it.draw_list.label(),
                        it.recorded_commands,
                        it.draw_calls,
                        it.indexed_draw_calls,
                        it.skipped_commands,
                        it.viewport_sets,
                        it.scissor_sets,
                        it.pipeline_binds,
                        it.vertex_buffer_binds,
                        it.index_buffer_binds,
                        it.bind_group_binds,
                        it.invalid_draw_calls,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            log::debug!("render frame graph v3: draw-list replay stats [{}]", draw_lists);
        }
    }
    Ok(report)
}

#[inline]
fn record_draw_list<T>(
    r: &mut dyn RenderApi,
    kind: RenderDrawListKind,
    record: impl FnOnce(&mut dyn RenderApi) -> EngineResult<T>,
) -> EngineResult<T> {
    r.begin_draw_list(kind)?;
    let record_result = record(r);
    let end_result = r.end_draw_list();
    match (record_result, end_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

impl RuntimeRenderController {
    #[inline]
    fn read_window_size<E: Send>(ctx: &ModuleCtx<'_, E>) -> (u32, u32) {
        ctx.resources()
            .get::<WindowInitSize>()
            .map(|s| (s.width, s.height))
            .unwrap_or((0, 0))
    }

    fn resize_if_needed(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        w: u32,
        h: u32,
    ) -> EngineResult<()> {
        if w == 0 || h == 0 {
            self.last_w = w;
            self.last_h = h;
            return Ok(());
        }

        // The render backend is initialized from the platform window snapshot before the
        // first editor frame is rendered. Calling `resize()` again on frame #0 can force
        // some Vulkan backends/drivers through a premature swapchain teardown/recreate
        // path before the first acquire. On older Windows/NVIDIA stacks this manifested
        // as a native access violation immediately after `render begin`.
        //
        // Therefore the first non-zero size is adopted as the backend-owned bootstrap
        // surface size. Real resize events after the first frame still go through
        // `RenderApi::resize` below.
        if self.last_w == 0 || self.last_h == 0 {
            self.last_w = w;
            self.last_h = h;
            log::debug!(
                "render controller: adopted initial surface size {}x{}; skip first explicit resize",
                w,
                h
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: adopted initial surface size {}x{}; skip first resize",
                w, h
            ));
            return Ok(());
        }

        if w != self.last_w || h != self.last_h {
            let old_w = self.last_w;
            let old_h = self.last_h;
            log::debug!(
                "render controller: resize requested {}x{} -> {}x{}",
                old_w,
                old_h,
                w,
                h
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: resize requested {}x{} -> {}x{}",
                old_w, old_h, w, h
            ));

            r.resize(w, h)?;

            self.last_w = w;
            self.last_h = h;
            log::debug!("render controller: resize completed {}x{}", w, h);
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: resize completed {}x{}",
                w, h
            ));
        }
        Ok(())
    }


    fn pump_previews_fail_soft(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        dt: f32,
    ) {
        if self.previews_disabled {
            return;
        }

        let result = {
            let mut previews = self.previews.lock();
            previews.pump(r, dt)
        };

        if let Err(e) = result {
            self.previews_disabled = true;
            log::warn!(
                "render controller: primitive previews disabled for this session: {}",
                e
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: primitive previews disabled: {}",
                e
            ));
        }
    }

    fn disable_viewport_pass(&mut self, phase: &'static str, error: impl std::fmt::Display) {
        if !self.viewport_pass_disabled {
            log::error!(
                "render controller: viewport GPU pass disabled at {}: {}",
                phase,
                error
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: viewport pass disabled at {}: {}",
                phase,
                error
            ));
        }
        self.viewport_pass_disabled = true;
    }

    #[inline]
    fn sync_cursor_state<E: Send>(&mut self, ctx: &ModuleCtx<'_, E>, desired: CursorState) {
        if desired == self.last_cursor_state {
            return;
        }
        self.last_cursor_state = desired;
        let _ = ctx
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Cursor(desired)));
    }
}

impl<E: Send + 'static> Module<E> for RuntimeRenderController {
    fn id(&self) -> &'static str {
        "engine.render_controller"
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        // Loading-screen warmup: bake/load editor shaders and create the core lit pipelines
        // before the first playable frame reaches the draw loop. If the renderer is not
        // bound yet, render() still has the same recovery path as before.
        if let Ok(api) = require_render_api(ctx) {
            let mut r = api.lock();
            if let Err(e) = ensure_lit_pipeline(&mut self.lit, &mut **r) {
                log::warn!("render controller: loading-screen pipeline warmup skipped: {}", e);
            } else {
                let _ = r.pump_uploads(newengine_core::render::UploadPumpDesc::loading_screen_warmup());
            }
        }
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        // Shutdown must not call `end_frame()` unconditionally: on a normal close
        // there is no active frame, and older Vulkan drivers may crash on a
        // redundant present/submit path. Every render path is responsible for
        // closing its own frame before returning.
        newengine_core::crash::record_breadcrumb("render controller: shutdown begin".to_string());
        self.sync_cursor_state(ctx, CursorState::released());
        self.viewport_pass_disabled = true;
        self.previews_disabled = true;
        Ok(())
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();

        let plugin_snapshot_for_render = ctx
            .resources()
            .get::<newengine_plugin_host::PluginsSnapshot>()
            .cloned();
        if let Some(snap) = plugin_snapshot_for_render.as_ref() {
            self.plugins_bridge.publish(snap.clone());
        }
        if let Some(q) = ctx
            .resources_mut()
            .get_mut::<newengine_plugin_host::PluginControlQueue>()
        {
            for cmd in self.plugins_bridge.drain_cmds() {
                q.push(cmd);
            }
        }

        let (w, h) = Self::read_window_size(ctx);

        let backend_work_budget = if let Some(cfg) = ctx.resources().get::<crate::render_runtime::ResolvedRenderBackendConfig>() {
            self.clear_color = cfg.clear_color;
            Some(cfg.work_budget)
        } else {
            None
        };

        let trace_frame = self.frame_index < 8 || self.frame_index % 120 == 0;
        let api = match require_render_api(ctx) {
            Ok(api) => api,
            Err(_) => return Ok(()),
        };
        let mut r = api.lock();
        if let Some(budget) = backend_work_budget {
            let _ = r.set_work_budget(budget);
        }
        let material_upload_jobs = backend_work_budget
            .map(|b| b.max_upload_jobs_per_frame.max(1))
            .unwrap_or(1);
        self.pump_material_texture_requests(&mut **r, material_upload_jobs);

        if trace_frame {
            log::debug!("render controller: render begin next_frame={} window={}x{} viewport={}x{}", self.frame_index.saturating_add(1), w, h, self.viewport_bridge.read_extent().0, self.viewport_bridge.read_extent().1);
            newengine_core::crash::record_breadcrumb(format!("render controller: render begin next_frame={} window={}x{}", self.frame_index.saturating_add(1), w, h));
        }

        self.resize_if_needed(&mut **r, w, h)?;

        let (requested_vp_w, requested_vp_h) = self.viewport_bridge.read_extent();
        let direct_surface_viewport = ui.is_none()
            && requested_vp_w == 0
            && requested_vp_h == 0
            && w > 0
            && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };
        if trace_frame {
            log::debug!(
                "render controller: begin_frame next_frame={} clear={:.3},{:.3},{:.3},{:.3} viewport={}x{}",
                self.frame_index.saturating_add(1),
                self.clear_color[0],
                self.clear_color[1],
                self.clear_color[2],
                self.clear_color[3],
                vp_w,
                vp_h
            );
            newengine_core::crash::record_breadcrumb(format!("render controller: begin_frame next_frame={} clear={:.3},{:.3},{:.3},{:.3} viewport={}x{}", self.frame_index.saturating_add(1), self.clear_color[0], self.clear_color[1], self.clear_color[2], self.clear_color[3], vp_w, vp_h));
        }
        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;
        if trace_frame {
            log::debug!(
                "render controller: begin_frame completed frame={}",
                self.frame_index.saturating_add(1)
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: begin_frame completed frame={}",
                self.frame_index.saturating_add(1)
            ));
        }

        self.frame_index = self.frame_index.saturating_add(1).max(1);

        let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);
        self.overlay_metrics.begin_frame(dt);
        self.pump_previews_fail_soft(&mut **r, dt);
        let ui_enabled = ui.is_some();
        let mut frame_debug_snapshot: Option<RenderFrameDebugSnapshot> = None;

        if vp_w > 0 && vp_h > 0 && !self.viewport_pass_disabled {
            let extent = Extent2D::new(vp_w, vp_h);
            let rt = if direct_surface_viewport {
                None
            } else {
                match self.ensure_viewport_rt(&mut **r, extent) {
                    Ok(rt) => Some(rt),
                    Err(e) => {
                        self.disable_viewport_pass("ensure_viewport_rt", &e);
                        if let Some(ui) = ui {
                            r.set_ui_draw_list(ui);
                        }
                        self.gc_per_draw_ubos(&mut **r);
                        self.gc_deferred_rts(&mut **r);
                        if trace_frame {
                            newengine_core::crash::record_breadcrumb(format!(
                                "render controller: end_frame frame={} after viewport RT failure",
                                self.frame_index
                            ));
                        }
                        r.end_frame()?;
                        return Ok(());
                    }
                }
            };

            let input = if direct_surface_viewport {
                ViewportInputSnap::read_direct_surface(
                    ctx.resources().get::<newengine_ui::UiInputFrame>(),
                )
            } else {
                ViewportInputSnap::read(&self.viewport_bridge)
            };

            self.scene_bridge.apply_commands();
            let play_mode = self.scene_bridge.play_mode();
            let mut nav_input = CameraNavInput {
                dx_px: input.dx_px,
                dy_px: input.dy_px,
                wheel_y: input.wheel_y,
                active: input.active,
                look_drag: input.look_drag,
                pan_drag: input.pan_drag,
                ui_busy: input.ui_busy,
                fly_rmb: input.fly_rmb,
                move_mask: input.move_mask,
                speed_scalar: input.speed_scalar,
            };

            if play_mode.wants_direct_player_control() {
                nav_input.wheel_y = 0.0;
                nav_input.pan_drag = false;
            }

            let aspect = (vp_w as f32 / vp_h as f32).max(1e-6);

            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();

            // Single source of truth: scene drives tick phasing + derived updates.
            // Pre-pass provides bounds/world poses for controller logic.
            // Post-pass commits camera/nav writes into derived outputs for rendering.
            let (rig, viewproj, effective_play_mode, _world_playable) = scene.run_frame(self.frame_index, |world| {
                let cam_id = world
                    .resource::<newengine_scene::SceneState>()
                    .and_then(|s| s.active_camera.or(s.root))
                    .unwrap_or_default();

                let world_playable = readiness::update_game_ready_launch_gate(
                    self,
                    &mut **r,
                    world,
                    play_mode,
                    self.frame_index,
                );
                let effective_play_mode = if world_playable {
                    play_mode
                } else {
                    EditorPlayMode::Edit
                };

                if self.last_play_mode != effective_play_mode {
                    if !self.last_play_mode.is_runtime() && effective_play_mode.is_runtime() {
                        self.runtime_session = Some(capture_runtime_world_snapshot(world));
                    }

                    if self.last_play_mode.wants_direct_player_control() {
                        if let Some(player) = first_player(world) {
                            clear_player_input(world, player);
                        }

                        detach_active_camera_from_player(world, cam_id);

                        if let Some(snapshot) = self.play_session.take() {
                            let _ = world.insert(snapshot.cam_id, snapshot.rig);
                            if let Some(transform) = snapshot.transform {
                                let _ = world.insert(snapshot.cam_id, transform);
                            }
                        }
                    }

                    if effective_play_mode.wants_direct_player_control() {
                        let rig = world
                            .get::<newengine_sim::CameraRigComp>(cam_id)
                            .copied()
                            .unwrap_or_default();
                        let transform = world.get::<newengine_transform::Transform>(cam_id).copied();
                        self.play_session = Some(super::controller::PlaySessionSnapshot {
                            cam_id,
                            rig,
                            transform,
                        });

                        if let Some(player) = first_player(world) {
                            attach_active_camera_to_player(world, cam_id, player);
                        }
                    } else {
                        detach_active_camera_from_player(world, cam_id);
                    }

                    if self.last_play_mode.is_runtime() && !effective_play_mode.is_runtime() {
                        if let Some(snapshot) = self.runtime_session.take() {
                            restore_runtime_world_snapshot(world, snapshot);
                        }
                    }

                    self.last_play_mode = effective_play_mode;
                }

                if let Some(player) = first_player(world) {
                    if effective_play_mode.wants_direct_player_control() {
                        apply_player_input(
                            world,
                            player,
                            input.move_mask,
                            Vec2::new(-input.dx_px, -input.dy_px),
                            input.active,
                        );
                    } else {
                        clear_player_input(world, player);
                    }
                }

                if effective_play_mode.runs_physics() {
                    run_schedule(&mut self.sim_schedule, world, dt);
                }

                let bounds = scene::scene_bounds_world(world).unwrap_or_else(|| scene::default_bounds());
                let sel = self.scene_bridge.selection();
                let sel_bounds = scene::selection_bounds_world(world, sel);

                let params = CameraNavParams {
                    dt,
                    aspect,
                    bounds: CamBoundsSphere {
                        center: bounds.center,
                        radius: bounds.radius,
                    },
                    selection_bounds: sel_bounds.map(|b| CamBoundsSphere {
                        center: b.center,
                        radius: b.radius,
                    }),
                };

                let frame_req = CameraNavFrameRequest {
                    seq: self.viewport_bridge.read_frame_request(),
                    all: self.viewport_bridge.read_frame_all(),
                };

                if effective_play_mode.wants_direct_player_control() {
                    nav_input.active = false;
                    nav_input.look_drag = false;
                    nav_input.pan_drag = false;
                    nav_input.fly_rmb = false;
                    nav_input.move_mask = 0;
                }

                let out = step_camera_nav(
                    &mut self.camera_nav,
                    world,
                    cam_id,
                    &mut nav_input,
                    params,
                    frame_req,
                );

                let rig = out.rig;
                self.projection = out.projection;

                self.last_aspect = aspect;
                self.last_vp_w = vp_w;
                self.last_vp_h = vp_h;

                let proj = self.projection.matrix();
                let view = rig.view_matrix();
                let viewproj = proj * view;

                (rig, viewproj, effective_play_mode, world_playable)
            });

            let desired_cursor_state = if effective_play_mode.wants_direct_player_control() && input.active {
                CursorState::captured_locked()
            } else {
                cursor_state_for_nav(&nav_input)
            };

            // AAA camera policy: editor fly uses RMB capture, while play mode possesses the
            // player directly only after the standalone scene launch gate is released.
            self.sync_cursor_state(ctx, desired_cursor_state);

            let proj = self.projection.matrix();
            let view = rig.view_matrix();

            passes::publish_camera_spawn(&self.viewport_bridge, &rig);
            self.viewport_bridge
                .publish_camera_frame(view, proj, vp_w, vp_h);

            picking::handle_picking(self, &scene, viewproj, vp_w, vp_h);

            // Bounds used below are now up-to-date after the scene post-pass.
            let bounds = scene::scene_bounds(&scene).unwrap_or_else(|| scene::default_bounds());

            let lit = match ensure_lit_pipeline(&mut self.lit, &mut **r) {
                Ok(lit) => lit,
                Err(e) => {
                    self.disable_viewport_pass("ensure_lit_pipeline", &e);
                    // Keep the swapchain/UI alive. The 3D viewport can recover on next launch
                    // after shader cache/toolchain issues are fixed.
                    r.set_viewport(Viewport::full(Extent2D::new(w, h)))?;
                    r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;
                    if let Some(ui) = ui {
                        r.set_ui_draw_list(ui);
                    }
                    self.gc_per_draw_ubos(&mut **r);
                    self.gc_deferred_rts(&mut **r);
                    if trace_frame {
                        newengine_core::crash::record_breadcrumb(format!(
                            "render controller: end_frame frame={} after viewport disable",
                            self.frame_index
                        ));
                    }
                    r.end_frame()?;
                    return Ok(());
                }
            };
            let base_lights = lights::collect_lights(scene.world());
            let shadow_plan = match shadows::build_light_shadow_plan(
                self,
                &mut **r,
                &scene,
                bounds,
                lit,
                viewproj,
                [rig.position.x, rig.position.y, rig.position.z],
                extent,
                Extent2D::new(w, h),
                plugin_snapshot_for_render.as_ref(),
            ) {
                Ok(plan) => plan,
                Err(e) => {
                    log::warn!("render controller: shadow plan disabled for this frame: {}", e);
                    let _ = r.discard_recorded_commands();
                    shadows::LightShadowPlan::disabled(lit.white_texture)
                }
            };
            if trace_frame {
                let shadow_kind = shadow_plan
                    .light_kind
                    .map(|kind| kind.label())
                    .unwrap_or("none");
                log::debug!(
                    "render shadow plan: kind={} active={} target={:?} resolution={}",
                    shadow_kind,
                    shadow_plan.is_active(),
                    shadow_plan.render_target(),
                    shadow_plan.resolution
                );
            }
            let shadow_frame = shadow_plan.frame;
            let world_lights = base_lights.with_shadow(shadow_frame.light_mvp, shadow_frame.params);
            let win_extent = Extent2D::new(w, h);

            let extraction = SceneExtractionCtx {
                scene: &scene,
                lit,
                viewproj,
                rig: &rig,
                bounds,
                lights: world_lights,
                shadow_plan,
                shadow_frame,
                viewport_extent: extent,
                surface_extent: win_extent,
                runtime: effective_play_mode.is_runtime(),
                editor_overlays: !effective_play_mode.is_runtime() && !play_mode.is_runtime(),
                ui: ui.as_ref(),
            };
            let mut provider_registry = standard_runtime_draw_list_provider_registry();
            if let Some(snapshot) = plugin_snapshot_for_render.as_ref() {
                provider_registry.sync_plugin_capabilities(snapshot);
            }
            if trace_frame {
                log::debug!(
                    "render draw-list providers: {}",
                    provider_registry.labels().join(",")
                );
            }
            let providers = provider_registry.providers();
            let visibility = extraction.visibility();
            let mut draw_lists = RuntimeDrawListSet::extract(
                visibility,
                &extraction,
                providers.as_slice(),
            );
            provider_registry.add_external_draw_lists(visibility, &mut draw_lists);

            let provider_result = {
                let mut build_ctx = DrawListBuildCtx::new(self, &mut **r, &draw_lists);
                draw_lists.record_pass_state(&extraction, &mut build_ctx)
                    .and_then(|()| {
                        for provider in providers.iter().copied() {
                            provider.extract(&extraction, &mut build_ctx)?;
                        }
                        Ok(())
                    })
            };
            if let Err(e) = provider_result {
                self.disable_viewport_pass("draw_list.provider_extraction", &e);
                let _ = r.discard_recorded_commands();
                if let Some(ui) = ui.as_ref() {
                    r.set_ui_draw_list(ui.clone());
                }
                if trace_frame {
                    newengine_core::crash::record_breadcrumb(format!(
                        "render controller: end_frame frame={} after draw-list provider failure",
                        self.frame_index
                    ));
                }
                r.end_frame()?;
                return Ok(());
            }

            let shadow_rt_for_graph = shadow_plan.render_target();
            let frame_plan = standard_runtime_frame(
                StandardRuntimePipelineDesc::new(
                    self.frame_index,
                    Extent2D::new(w, h),
                    extent,
                )
                .viewport_is_surface(direct_surface_viewport)
                .viewport_render_target(rt)
                .shadow(shadow_rt_for_graph.is_some(), shadow_plan.resolution)
                .shadow_render_target(shadow_rt_for_graph)
                .deferred(false)
                .postfx(false)
                .ui(ui_enabled)
                .debug_overlay(true)
                .draw_lists(draw_lists.descriptors()),
            );

            provider_registry.validate_routes(&frame_plan.validate_draw_list_routes())?;
            {
                let mut build_ctx = DrawListBuildCtx::new(self, &mut **r, &draw_lists);
                provider_registry.extract_external_providers(
                    &extraction,
                    &draw_lists,
                    &frame_plan,
                    &mut build_ctx,
                )?;
            }

            if trace_frame {
                let phases = frame_plan
                    .phase_order()
                    .map(|phase| phase.label())
                    .collect::<Vec<_>>()
                    .join(" -> " );
                log::debug!("render frame graph v3: frame={} phases={}", self.frame_index, phases);
            }
            let submit_report = match submit_frame_plan_v3(&mut **r, &frame_plan, trace_frame) {
                Ok(report) => report,
                Err(e) => {
                    let _ = r.discard_recorded_commands();
                    let _ = r.end_frame();
                    return Err(e);
                }
            };
            self.overlay_metrics.record_graph_submit(submit_report.clone());
            frame_debug_snapshot = Some(RenderFrameDebugSnapshot {
                frame_index: self.frame_index,
                surface_extent: [w, h],
                viewport_extent: [vp_w, vp_h],
                direct_surface_viewport,
                graph_label: frame_plan.graph.label.clone().unwrap_or_else(|| "<unnamed>".to_owned()),
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
                notes: Vec::new(),
            });
        } else {
            // Viewport is not active. Keep cursor released and let the legacy UI
            // composite path keep the window responsive until a viewport exists.
            self.sync_cursor_state(ctx, CursorState::released());
            if let Some(ui) = ui {
                let win_extent = Extent2D::new(w, h);
                r.set_viewport(Viewport::full(win_extent))?;
                r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;
                r.set_ui_draw_list(ui);
            }
        }

        if let Ok(diag) = r.diagnostics_snapshot() {
            self.overlay_metrics.record_backend_snapshot(&diag);
            if let Some(snapshot) = frame_debug_snapshot.as_mut() {
                snapshot.queued_upload_jobs = diag.queue.queued_upload_jobs;
                snapshot.queued_upload_bytes = diag.queue.queued_upload_bytes;
                snapshot.resource_buffers = diag.resources.buffers;
                snapshot.resource_textures = diag.resources.textures;
                snapshot.resource_pipelines = diag.resources.pipelines;
            }
        }

        r.set_debug_text(self.overlay_metrics.overlay_text());

        self.gc_per_draw_ubos(&mut **r);
        self.gc_deferred_rts(&mut **r);
        if trace_frame {
            newengine_core::crash::record_breadcrumb(format!("render controller: end_frame frame={}", self.frame_index));
        }
        r.end_frame()?;
        let telemetry_to_publish = if let Some(snapshot) = frame_debug_snapshot.take() {
            self.overlay_metrics.publish_debug_snapshot(snapshot);
            Some(self.overlay_metrics.telemetry_snapshot())
        } else {
            None
        };
        if trace_frame {
            if let Ok(diag) = r.diagnostics_snapshot() {
                log::debug!(
                    "render diagnostics: frame={} begin_ms={:.3} end_ms={:.3} upload_ms={:.3} pipeline_ms={:.3} buffers={} textures={} pipelines={} upload_jobs={} upload_mb={:.2} queued_uploads={} queued_mb={:.2}",
                    diag.frame.frame_index,
                    diag.frame.last_begin_frame_ms,
                    diag.frame.last_end_frame_ms,
                    diag.frame.last_blocking_upload_ms,
                    diag.frame.last_pipeline_build_ms,
                    diag.resources.buffers,
                    diag.resources.textures,
                    diag.resources.pipelines,
                    diag.queue.blocking_upload_jobs,
                    diag.queue.blocking_upload_bytes as f32 / (1024.0 * 1024.0),
                    diag.queue.queued_upload_jobs,
                    diag.queue.queued_upload_bytes as f32 / (1024.0 * 1024.0),
                );
            }
        }
        drop(r);
        if let Some(telemetry) = telemetry_to_publish {
            ctx.resources_mut().insert(telemetry);
        }
        Ok(())
    }
}

