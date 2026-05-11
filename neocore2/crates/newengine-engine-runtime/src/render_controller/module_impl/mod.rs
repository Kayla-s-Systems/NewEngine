#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera_runtime::{
    cursor_state_for_nav, step_camera_nav, BoundsSphere as CamBoundsSphere, CameraNavFrameRequest,
    CameraNavInput, CameraNavParams,
};
use newengine_core::host_events::CursorState;
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, Extent2D, RectI32, RenderBackendStatus, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_math::{Quat, Vec2, Vec3};
use newengine_ui::draw::UiDrawList;

use super::controller::RuntimeRenderController;
use super::gpu::ensure_lit_pipeline;
use crate::gameplay::{
    apply_player_input, attach_active_camera_to_player, capture_runtime_world_snapshot,
    clear_player_input, detach_active_camera_from_player, first_player, restore_runtime_world_snapshot,
    run_schedule,
};

mod grid;
mod input;
mod lights;
mod passes;
mod passes_ubo;
mod picking;
mod previews;
mod scene;
mod shadows;
mod windowing;

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

        if self.backend_render_disabled() {
            ctx.resources_mut().insert::<RenderBackendStatus>(self.backend_failure.snapshot());
            self.sync_cursor_state(ctx, CursorState::released());
            return Ok(());
        }

        if let Some(snap) = ctx
            .resources()
            .get::<newengine_plugin_host::PluginsSnapshot>()
        {
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
            Ok(api) => api.clone(),
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

        if let Err(error) = self.resize_if_needed(&mut **r, w, h) {
            return self.record_render_backend_error("resize", error);
        }

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
        if let Err(error) = r.begin_frame(BeginFrameDesc::new(self.clear_color)) {
            return self.record_render_backend_error("begin_frame", error);
        }
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
                        self.record_render_backend_result("end_frame_after_viewport_rt_failure", r.end_frame())?;
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

            let desired_cursor_state = if play_mode.wants_direct_player_control() && input.active {
                CursorState::captured_locked()
            } else {
                cursor_state_for_nav(&nav_input)
            };

            // AAA camera policy: editor fly uses RMB capture, while play mode possesses the
            // player directly and keeps the cursor locked without an extra mouse chord.
            self.sync_cursor_state(ctx, desired_cursor_state);

            let aspect = (vp_w as f32 / vp_h as f32).max(1e-6);

            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();

            // Single source of truth: scene drives tick phasing + derived updates.
            // Pre-pass provides bounds/world poses for controller logic.
            // Post-pass commits camera/nav writes into derived outputs for rendering.
            let (rig, viewproj) = scene.run_frame(self.frame_index, |world| {
                let cam_id = world
                    .resource::<newengine_scene::SceneState>()
                    .and_then(|s| s.active_camera.or(s.root))
                    .unwrap_or_default();

                if self.last_play_mode != play_mode {
                    if !self.last_play_mode.is_runtime() && play_mode.is_runtime() {
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

                    if play_mode.wants_direct_player_control() {
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

                    if self.last_play_mode.is_runtime() && !play_mode.is_runtime() {
                        if let Some(snapshot) = self.runtime_session.take() {
                            restore_runtime_world_snapshot(world, snapshot);
                        }
                    }

                    self.last_play_mode = play_mode;
                }

                if let Some(player) = first_player(world) {
                    if play_mode.wants_direct_player_control() {
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

                if play_mode.runs_physics() {
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

                if play_mode.wants_direct_player_control() {
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

                (rig, viewproj)
            });

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
                    self.record_render_backend_result("end_frame_after_viewport_disable", r.end_frame())?;
                    return Ok(());
                }
            };
            let base_lights = lights::collect_lights(scene.world());
            let shadow_frame = match shadows::prepare_shadow_frame(
                self,
                &mut **r,
                &scene,
                bounds,
                lit,
                play_mode.is_runtime(),
            ) {
                Ok(frame) => frame,
                Err(e) => {
                    log::warn!("render controller: shadow pass disabled for this frame: {}", e);
                    shadows::ShadowFrame::disabled(lit.white_texture)
                }
            };
            let world_lights = base_lights.with_shadow(shadow_frame.light_mvp, shadow_frame.params);

            let viewport_draw = (|| -> EngineResult<()> {
                if trace_frame {
                    newengine_core::crash::record_breadcrumb(format!(
                        "render controller: begin_render_target frame={} rt={}x{}",
                        self.frame_index, vp_w, vp_h
                    ));
                }
                if let Some(rt) = rt {
                    r.begin_render_target(
                        BeginRenderTargetDesc::new(rt)
                            .with_clear_depth(1.0)
                            .with_clear_color(grid::BACKGROUND_COLOR),
                    )?;
                }
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

                if !play_mode.is_runtime() {
                    passes::draw_grid(
                        self,
                        &mut **r,
                        lit,
                        viewproj,
                        &rig,
                        bounds.radius,
                        &world_lights,
                    )?;
                }
                passes::draw_procedural_terrain(
                    self,
                    &mut **r,
                    &scene,
                    lit,
                    viewproj,
                    &world_lights,
                    shadow_frame.texture,
                    play_mode.is_runtime(),
                )?;
                passes::draw_primitives(
                    self,
                    &mut **r,
                    &scene,
                    lit,
                    viewproj,
                    &world_lights,
                    shadow_frame.texture,
                    play_mode.is_runtime(),
                )?;
                if !play_mode.is_runtime() {
                    passes::draw_light_gizmos(
                        self,
                        &mut **r,
                        &scene,
                        lit,
                        viewproj,
                        &world_lights,
                        quat_from_forward_z,
                        false,
                    )?;
                    passes::draw_collision_wireframe(self, &mut **r, &scene, viewproj)?;
                }

                if rt.is_some() {
                    r.end_render_target()?;
                }
                Ok(())
            })();

            if let Err(e) = viewport_draw {
                self.disable_viewport_pass("viewport_draw", &e);
                // `end_frame()` is intentionally still called: it closes any active
                // render pass and presents a safe blank/UI frame instead of exiting
                // with a half-recorded Vulkan command buffer.
                if let Some(ui) = ui {
                    r.set_ui_draw_list(ui);
                }
                if trace_frame {
                    newengine_core::crash::record_breadcrumb(format!(
                        "render controller: end_frame frame={} after viewport draw failure",
                        self.frame_index
                    ));
                }
                self.record_render_backend_result("end_frame_after_viewport_draw_failure", r.end_frame())?;
                return Ok(());
            }

            let win_extent = Extent2D::new(w, h);
            r.set_viewport(Viewport::full(win_extent))?;
            r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;
        } else {
            // Viewport is not active (no RT). Ensure we don't keep the cursor captured.
            self.sync_cursor_state(ctx, CursorState::released());
        }

        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }

        ctx.resources_mut().insert(self.overlay_metrics.snapshot(self.frame_index));
        ctx.resources_mut().insert::<RenderBackendStatus>(RenderBackendStatus::healthy());

        self.gc_per_draw_ubos(&mut **r);
        self.gc_deferred_rts(&mut **r);
        if trace_frame {
            newengine_core::crash::record_breadcrumb(format!("render controller: end_frame frame={}", self.frame_index));
        }
        if let Err(error) = r.end_frame() {
            return self.record_render_backend_error("end_frame", error);
        }
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
        Ok(())
    }
}

