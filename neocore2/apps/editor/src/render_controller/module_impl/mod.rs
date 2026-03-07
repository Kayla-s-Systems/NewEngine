#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera_runtime::{
    cursor_state_for_nav, step_camera_nav, BoundsSphere as CamBoundsSphere, CameraNavFrameRequest,
    CameraNavInput, CameraNavParams,
};
use newengine_core::host_events::WindowInitSize;
use newengine_core::host_events::{CursorState, HostEvent, WindowHostEvent};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, Extent2D, RectI32, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_math::{Quat, Vec3};
use newengine_ui::draw::UiDrawList;

use super::controller::EditorRenderController;
use super::gpu::ensure_lit_pipeline;

mod input;
mod lights;
mod passes;
mod passes_ubo;
mod picking;
mod scene;

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

impl EditorRenderController {
    pub(super) const GRID_HALF_LINES: i32 = 80;
    pub(super) const GRID_MAJOR_EVERY: i32 = 10;
    pub(super) const GRID_MINOR_COLOR: [f32; 4] = [0.32, 0.32, 0.34, 1.0];
    pub(super) const GRID_MAJOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
    pub(super) const GRID_BACKGROUND_COLOR: [f32; 4] = [0.10, 0.10, 0.11, 1.0];

    #[inline]
    pub(super) fn grid_spacing(camera_distance: f32) -> f32 {
        let d = camera_distance.max(0.01);
        let base = (d * 0.08).max(0.05);
        let pow10 = 10.0f32.powf(base.log10().floor());
        pow10.clamp(0.05, 1000.0)
    }

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
        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            r.resize(w, h)?;
        }
        Ok(())
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

impl<E: Send + 'static> Module<E> for EditorRenderController {
    fn id(&self) -> &'static str {
        "app.render_controller"
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();

        if let Some(snap) = ctx
            .resources()
            .get::<newengine_core::plugins::PluginsSnapshot>()
        {
            self.plugins_bridge.publish(snap.clone());
        }
        if let Some(q) = ctx
            .resources_mut()
            .get_mut::<newengine_core::plugins::PluginControlQueue>()
        {
            for cmd in self.plugins_bridge.drain_cmds() {
                q.push(cmd);
            }
        }

        let (w, h) = Self::read_window_size(ctx);

        let api = match require_render_api(ctx) {
            Ok(api) => api,
            Err(_) => return Ok(()),
        };
        let mut r = api.lock();

        self.resize_if_needed(&mut **r, w, h)?;

        let (vp_w, vp_h) = self.viewport_bridge.read_extent();
        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;

        self.frame_index = self.frame_index.saturating_add(1).max(1);

        let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);
        {
            let mut p = self.previews.lock();
            p.pump(&mut **r, dt)?;
        }

        if vp_w > 0 && vp_h > 0 {
            let extent = Extent2D::new(vp_w, vp_h);
            let rt = self.ensure_viewport_rt(&mut **r, extent)?;

            let input = ViewportInputSnap::read(&self.viewport_bridge);

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
            };

            // AAA camera policy: when free-fly is active and viewport is active, capture cursor.
            self.sync_cursor_state(ctx, cursor_state_for_nav(&nav_input));

            let aspect = (vp_w as f32 / vp_h as f32).max(1e-6);

            self.scene_bridge.apply_commands();
            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();

            // Single source of truth: scene drives tick phasing + derived updates.
            // Pre-pass provides bounds/world poses for controller logic.
            // Post-pass commits camera/nav writes into derived outputs for rendering.
            let (rig, viewproj) = scene.run_frame(self.frame_index, |world| {
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

                let cam_id = world
                    .resource::<newengine_scene::SceneState>()
                    .and_then(|s| s.active_camera.or(s.root))
                    .unwrap_or_default();

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

            r.begin_render_target(
                BeginRenderTargetDesc::new(rt)
                    .with_clear_depth(1.0)
                    .with_clear_color(Self::GRID_BACKGROUND_COLOR),
            )?;
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

            let lit = ensure_lit_pipeline(&mut self.lit, &mut **r)?;
            let world_lights = lights::collect_lights(scene.world());

            passes::draw_grid(
                self,
                &mut **r,
                lit,
                viewproj,
                &rig,
                bounds.center,
                bounds.radius,
                &world_lights,
            )?;
            passes::draw_primitives(self, &mut **r, &scene, lit, viewproj, &world_lights)?;
            passes::draw_light_gizmos(
                self,
                &mut **r,
                &scene,
                lit,
                viewproj,
                &world_lights,
                quat_from_forward_z,
            )?;

            r.end_render_target()?;

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

        self.gc_per_draw_ubos(&mut **r);
        self.gc_deferred_rts(&mut **r);
        r.end_frame()?;
        Ok(())
    }
}

