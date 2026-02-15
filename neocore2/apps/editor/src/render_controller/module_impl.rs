#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Vec3};
use newengine_camera::{auto_near_far_from_sphere, orbit_frame_sphere, CameraInput, Perspective, Projection};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, BufferSlice, Extent2D, IndexFormat,
    RectI32, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

use newengine_primitives::Primitive;
use newengine_scene::{update_scene_world, SceneBounds};
use newengine_transform::GlobalTransform;

use super::controller::EditorRenderController;
use super::gpu::{ensure_grid, ensure_lit_pipeline, ensure_primitive_gpu};

impl EditorRenderController {
    #[inline]
    fn apply_wasd_target_translate(orbit: &mut newengine_camera::OrbitController, mask: u64, dt: f32, base_speed: f32) {
        if mask == 0 {
            return;
        }

        let mut dir = Vec3::ZERO;
        if (mask & (1 << 0)) != 0 {
            dir.z -= 1.0;
        }
        if (mask & (1 << 2)) != 0 {
            dir.z += 1.0;
        }
        if (mask & (1 << 1)) != 0 {
            dir.x -= 1.0;
        }
        if (mask & (1 << 3)) != 0 {
            dir.x += 1.0;
        }
        if (mask & (1 << 4)) != 0 {
            dir.y -= 1.0;
        }
        if (mask & (1 << 5)) != 0 {
            dir.y += 1.0;
        }

        if dir.length_squared() <= 1e-6 {
            return;
        }

        let speed_mul = if (mask & (1 << 6)) != 0 { 3.5 } else { 1.0 };
        let v = dir.normalize() * (base_speed * speed_mul * dt);
        orbit.target += v;
    }

    #[inline]
    fn write_mat4_ubo(
        r: &mut dyn newengine_core::render::RenderApi,
        ubo: newengine_core::render::BufferId,
        m: Mat4,
    ) -> EngineResult<()> {
        let cols = m.to_cols_array();
        let mut bytes: [u8; 64] = [0u8; 64];
        for (i, f) in cols.iter().enumerate() {
            let off = i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }
        r.write_buffer(ubo, 0, &bytes)
    }
}

impl<E: Send + 'static> Module<E> for EditorRenderController {
    fn id(&self) -> &'static str {
        "app.render_controller"
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();

        if let Some(snap) = ctx.resources().get::<newengine_core::plugins::PluginsSnapshot>() {
            self.plugins_bridge.publish(snap.clone());
        }

        let (w, h) = ctx
            .resources()
            .get::<WinitWindowInitSize>()
            .map(|s| (s.width, s.height))
            .unwrap_or((0, 0));

        let api = match require_render_api(ctx) {
            Ok(api) => api,
            Err(_) => return Ok(()),
        };

        let mut r = api.lock();

        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            r.resize(w, h)?;
        }

        let (vp_w, vp_h) = self.viewport_bridge.read_extent();
        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;

        if vp_w > 0 && vp_h > 0 {
            let extent = Extent2D::new(vp_w, vp_h);
            let rt = self.ensure_viewport_rt(&mut **r, extent)?;

            // Collect input.
            let (dx_px, dy_px, wheel_y, _hovered, dragging) = self.viewport_bridge.read_orbit_input();
            let move_mask = self.viewport_bridge.read_move_keys();
            let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);

            // Apply editor commands deterministically before rendering.
            self.scene_bridge.apply_commands();

            // Update world derived state.
            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();
            {
                let world_mut = scene.world_mut();
                update_scene_world(world_mut);
            }

            // Extract bounds.
            let (bounds_center, bounds_radius) = {
                let world = scene.world();
                if let Some(b) = world.resource::<SceneBounds>() {
                    if let Some(s) = b.sphere {
                        (s.center, s.radius.max(0.001))
                    } else {
                        self.default_bounds()
                    }
                } else {
                    self.default_bounds()
                }
            };

            // Translate orbit target in world space.
            let base_speed = (bounds_radius.max(0.01) * 2.0).clamp(0.5, 200.0);
            Self::apply_wasd_target_translate(&mut self.orbit, move_mask, dt, base_speed);

            // Apply orbit rotation + dolly.
            let input = CameraInput {
                look_active: dragging,
                look_delta: glam::Vec2::new(dx_px, -dy_px),
                move_axis: Vec3::ZERO,
                speed_mul: 1.0,
                zoom_delta: wheel_y,
            };
            let aspect = vp_w as f32 / (vp_h.max(1) as f32);

            // Sensitivities are expressed in "scene space" and must remain stable across different
            // scene scales and input devices. Mouse wheel is normalized in UI to wheel "notches";
            // here we map it to a deterministic dolly speed derived from current scene bounds.
            self.orbit.look_sens = 0.0045;
            self.orbit.dolly_speed = (bounds_radius * 0.25).clamp(0.05, 10.0);
            self.orbit.pan_speed = (bounds_radius * 0.0025).clamp(0.001, 1.0);

            self.orbit.apply(&mut self.rig, input, dt);

            // Keep camera framing correct when viewport aspect or scene bounds change.
            // Do not auto-frame while user is actively manipulating the view.
            let aspect_changed = (aspect - self.last_aspect).abs() > 0.0005
                || vp_w != self.last_vp_w
                || vp_h != self.last_vp_h;

            let bounds_changed = if self.framed_once {
                let center_shift = (bounds_center - self.last_bounds_center).length();
                let radius_ratio = bounds_radius / self.last_bounds_radius.max(0.001);
                center_shift > (self.last_bounds_radius * 0.25).max(0.05)
                    || radius_ratio > 1.25
                    || radius_ratio < 0.80
            } else {
                true
            };

            let user_busy = dragging || move_mask != 0;

            if !user_busy && (aspect_changed || bounds_changed) {
                let fovy = 60.0f32.to_radians();
                orbit_frame_sphere(
                    &mut self.orbit,
                    bounds_center,
                    bounds_radius,
                    fovy,
                    aspect,
                    1.15,
                );
                self.framed_once = true;
                self.last_bounds_center = bounds_center;
                self.last_bounds_radius = bounds_radius;
            }

            self.last_aspect = aspect;
            self.last_vp_w = vp_w;
            self.last_vp_h = vp_h;
            self.orbit.min_distance = (bounds_radius * 0.05).max(0.05);
            let (near, far) = auto_near_far_from_sphere(self.orbit.distance, bounds_radius);

            self.projection = Projection::Perspective(Perspective::new(
                60.0f32.to_radians(),
                aspect,
                near,
                far,
            ));

            let proj = self.projection.matrix();
            let view = self.rig.view_matrix();
            let viewproj = proj * view;

            r.begin_render_target(BeginRenderTargetDesc::new(rt))?;
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

            // Lit pipeline (UBO is per draw: MVP).
            let lit = ensure_lit_pipeline(&mut self.lit, &mut **r)?;

            // Grid: reuse the same UBO bind group (layout compatible).
            if bounds_radius.is_finite() {
                let g = ensure_grid(&mut self.grid, &mut **r, lit.bgl, bounds_radius, self.orbit.distance)?;
                // Camera MVP for grid.
                Self::write_mat4_ubo(&mut **r, lit.ubo, viewproj)?;

                r.set_pipeline(g.pipeline)?;
                r.set_bind_group(0, lit.bg)?;
                r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
                r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
            }

            // Primitives.
            {
                let world = scene.world();
                for (_id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
                    let gpu = ensure_primitive_gpu(prim.kind, &mut self.prim_cube, &mut self.prim_plane, &mut **r)?;

                    let mvp = viewproj * gt.0;
                    Self::write_mat4_ubo(&mut **r, lit.ubo, mvp)?;

                    r.set_pipeline(lit.pipeline)?;
                    r.set_bind_group(0, lit.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(gpu.index_count))?;
                }
            }

            r.end_render_target()?;

            let win_extent = Extent2D::new(w, h);
            r.set_viewport(Viewport::full(win_extent))?;
            r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;
        }

        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }

        r.end_frame()?;
        Ok(())
    }
}
