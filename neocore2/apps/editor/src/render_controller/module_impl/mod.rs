#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{auto_near_far_from_sphere, Perspective, Projection};
use newengine_core::host_events::{CursorState, HostEvent, WindowHostEvent};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, Extent2D, RectI32, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_math::{Quat, Vec3};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

use super::controller::EditorRenderController;
use super::gpu::ensure_lit_pipeline;
use newengine_core::MissingServicePolicy;
use newengine_scene::update_scene_world;
use newengine_transform_api::runtime::TransformRuntimeApi;
use newengine_transform_api::TRANSFORM_SERVICE;

mod camera;
mod input;
mod lights;
mod passes;
mod passes_ubo;
mod picking;
mod scene;

use camera::{CameraUpdateParams, CameraUpdateResult};
use input::ViewportInputSnap;
use scene::BoundsSnap;

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
            .get::<WinitWindowInitSize>()
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
    fn sync_camera_capture<E: Send>(&mut self, ctx: &ModuleCtx<'_, E>, want_capture: bool) {
        if want_capture == self.camera_capture_active {
            return;
        }
        self.camera_capture_active = want_capture;

        let state = if want_capture {
            CursorState::captured_locked()
        } else {
            CursorState::released()
        };

        let _ = ctx
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Cursor(state)));
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

            let mut input = ViewportInputSnap::read(&self.viewport_bridge);

            // AAA camera policy: when free-fly is active, capture cursor and switch to
            // relative look mode (cursor hidden + grabbed).
            self.sync_camera_capture(ctx, input.fly_rmb && input.active);

            let aspect = (vp_w as f32 / vp_h as f32).max(1e-6);

            let transform_api: Option<TransformRuntimeApi> = ctx
                .services()
                .service_registry()
                .require_interface::<TransformRuntimeApi>(
                    TRANSFORM_SERVICE,
                    MissingServicePolicy::Optional,
                );

            // Drive ECS change-tracking deterministically from the render/controller frame index.
            // Without this, `update_scene_world()` can stop propagating transforms/bounds, which
            // breaks camera/world pose stability across input mode transitions.
            {
                let scene_lock = self.scene_bridge.scene();
                let mut scene = scene_lock.write();
                scene.world_mut().set_tick(self.frame_index);
            }

            self.scene_bridge.apply_commands();
            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();

            {
                let world_mut = scene.world_mut();
                update_scene_world(world_mut);
            }

            let bounds = scene::scene_bounds(&scene).unwrap_or_else(|| scene::default_bounds());
            let sel = self.scene_bridge.selection();
            let sel_bounds = scene::selection_bounds(&scene, sel);

            let (user_busy, explicit_frame, frame_all) =
                self.scene_compute_framing_flags(&input, &bounds);

            let base_speed = (bounds.radius.max(0.01) * 2.0).clamp(0.5, 200.0);

            let params = CameraUpdateParams {
                dt,
                aspect,
                bounds,
                sel_bounds,
                base_speed,
                user_busy,
                explicit_frame,
                frame_all,
            };

            // IMPORTANT:
            // `update_scene_world()` advanced derived caches to `world.tick()`.
            // The camera writes `Transform` after this point. If we keep the same tick,
            // the change-tracking gate (`max_changed_tick > since_tick`) will never see
            // those writes (they would be == last_transform_tick).
            // Advance the tick once for the controller phase so camera pose commits are
            // visible to the next frame's `update_scene_world()`.
            scene.world_mut().advance_tick();

            let CameraUpdateResult { rig, .. } =
                camera::update_camera_and_persist(self, &mut scene, &mut input, params);

            self.last_bounds_center = params.bounds.center;
            self.last_bounds_radius = params.bounds.radius;

            self.last_aspect = aspect;
            self.last_vp_w = vp_w;
            self.last_vp_h = vp_h;

            let cam_dist = (rig.position - params.bounds.center).length().max(0.01);
            let (near, far) = auto_near_far_from_sphere(cam_dist, params.bounds.radius);
            self.projection =
                Projection::Perspective(Perspective::new(60.0f32.to_radians(), aspect, near, far));

            let proj = self.projection.matrix();
            let view = rig.view_matrix();
            let viewproj = proj * view;

            passes::publish_camera_spawn(&self.viewport_bridge, &rig);
            self.viewport_bridge
                .publish_camera_frame(view, proj, vp_w, vp_h);

            picking::handle_picking(self, &scene, viewproj, vp_w, vp_h);

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
                params.bounds.center,
                params.bounds.radius,
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
            self.sync_camera_capture(ctx, false);
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

impl EditorRenderController {
    #[inline]
    fn scene_compute_framing_flags(
        &mut self,
        input: &ViewportInputSnap,
        bounds: &BoundsSnap,
    ) -> (bool, bool, bool) {
        let bounds_center_delta = (bounds.center - self.last_bounds_center).length();
        let bounds_radius_delta = (bounds.radius - self.last_bounds_radius).abs();
        let eps = bounds.radius.max(0.001) * 0.0005;
        let bounds_changed = bounds_center_delta > eps || bounds_radius_delta > eps;

        let user_busy = input.look_drag
            || input.pan_drag
            || input.move_mask != 0
            || input.ui_busy
            || bounds_changed;

        let frame_seq = self.viewport_bridge.read_frame_request();
        let frame_all = self.viewport_bridge.read_frame_all();
        let explicit_frame = frame_seq != self.last_frame_seq;
        if explicit_frame {
            self.last_frame_seq = frame_seq;
        }

        (user_busy, explicit_frame, frame_all)
    }
}
