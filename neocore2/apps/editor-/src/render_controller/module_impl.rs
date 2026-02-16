#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Quat, Vec3};
use newengine_camera::{auto_near_far_from_sphere, orbit_frame_sphere, CameraInput, Perspective, Projection};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, BufferSlice, Extent2D, IndexFormat, RectI32, Viewport,
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
    // Editor "floor" constraint: camera must stay above this Y.
    const MIN_CAMERA_Y: f32 = 0.10;

    // Blender-like orbit constraints.
    const MAX_PITCH_ABS: f32 = 1.5184364; // ~87 deg
    const MIN_DISTANCE: f32 = 0.30;

    // Fit-to-bounds behavior: expand only when scene grows.
    const FRAME_GROWTH_EPS: f32 = 1.08;

    #[inline]
    fn enforce_orbit_basic(orbit: &mut newengine_camera::OrbitController) {
        orbit.distance = orbit.distance.max(Self::MIN_DISTANCE);
        orbit.pitch_limit = orbit.pitch_limit.min(Self::MAX_PITCH_ABS);

        // Allow pitch both directions, but keep away from singularities.
        orbit.pitch = orbit.pitch.clamp(-Self::MAX_PITCH_ABS, Self::MAX_PITCH_ABS);
    }

    #[inline]
    fn compute_rot(orbit: &newengine_camera::OrbitController) -> Quat {
        // We keep the convention consistent with our rig math:
        // orbit.pitch grows when looking "up" (mouse up), but to keep camera above Y,
        // we invert in rotation_x to keep the intuitive editor feel stable.
        //
        // The important part is not the sign itself, but that:
        // - mouse up increases pitch and raises camera around target
        // - floor constraint is achieved by lifting target, not clamping pitch
        let rot_yaw = Quat::from_rotation_y(orbit.yaw);
        let rot_pitch = Quat::from_rotation_x(-orbit.pitch);
        rot_yaw * rot_pitch
    }

    #[inline]
    fn sync_rig_with_floor_lift(
        orbit: &mut newengine_camera::OrbitController,
        rig: &mut newengine_camera::CameraRig,
    ) {
        Self::enforce_orbit_basic(orbit);

        // We solve the floor constraint by lifting the pivot (target),
        // which preserves Blender-like orbit behavior around objects.
        //
        // One lift is usually enough. Second pass makes it robust if pitch is extreme.
        for _ in 0..2 {
            let rot = Self::compute_rot(orbit);
            let pos = orbit.target + (rot * Vec3::Z) * orbit.distance;

            if pos.y >= Self::MIN_CAMERA_Y {
                rig.position = pos;
                rig.rotation = rot;
                return;
            }

            let dy = Self::MIN_CAMERA_Y - pos.y;
            orbit.target.y += dy;
        }

        // Final assignment (after lifts)
        let rot = Self::compute_rot(orbit);
        let mut pos = orbit.target + (rot * Vec3::Z) * orbit.distance;
        if pos.y < Self::MIN_CAMERA_Y {
            pos.y = Self::MIN_CAMERA_Y;
        }
        rig.position = pos;
        rig.rotation = rot;
    }

    #[inline]
    fn apply_move_axes(orbit: &mut newengine_camera::OrbitController, mask: u64, dt: f32, base_speed: f32) {
        if mask == 0 {
            return;
        }

        let mut dir = Vec3::ZERO;

        // 0..3: WASD
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

        // 4..5: vertical track (optional; bind in input layer if you want)
        if (mask & (1 << 4)) != 0 {
            dir.y += 1.0;
        }
        if (mask & (1 << 5)) != 0 {
            dir.y -= 1.0;
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

        // Apply plugin control commands produced by the editor UI.
        if let Some(q) = ctx.resources_mut().get_mut::<newengine_core::plugins::PluginControlQueue>() {
            for cmd in self.plugins_bridge.drain_cmds() {
                q.push(cmd);
            }
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

            let (dx_px, dy_px, wheel_y, _hovered, dragging) = self.viewport_bridge.read_orbit_input();
            let move_mask = self.viewport_bridge.read_move_keys();
            let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);

            self.scene_bridge.apply_commands();

            let scene_lock = self.scene_bridge.scene();
            let mut scene = scene_lock.write();
            {
                let world_mut = scene.world_mut();
                update_scene_world(world_mut);
            }

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

            let base_speed = (bounds_radius.max(0.01) * 2.0).clamp(0.5, 200.0);
            Self::apply_move_axes(&mut self.orbit, move_mask, dt, base_speed);

            let input = CameraInput {
                look_active: dragging,
                look_delta: glam::Vec2::new(dx_px, -dy_px),
                move_axis: Vec3::ZERO,
                speed_mul: 1.0,
                zoom_delta: wheel_y,
            };

            let aspect = vp_w as f32 / (vp_h as f32);

            // NOTE: fix typo; keep stable aspect even if vp_h==0 (we are in vp_h>0 branch)
            let aspect = vp_w as f32 / (vp_h as f32);

            self.orbit.look_sens = 0.0045;
            self.orbit.dolly_speed = (bounds_radius * 0.25).clamp(0.05, 10.0);
            self.orbit.pan_speed = (bounds_radius * 0.0025).clamp(0.001, 1.0);

            // Apply controller then enforce floor by lifting pivot (Blender-like).
            Self::enforce_orbit_basic(&mut self.orbit);
            self.orbit.apply(&mut self.rig, input, dt);
            Self::sync_rig_with_floor_lift(&mut self.orbit, &mut self.rig);

            // Framing:
            // - startup / aspect change: frame
            // - scene growth: expand only (never shrink -> no annoying zoom-in on spawn)
            let aspect_changed = (aspect - self.last_aspect).abs() > 0.0005
                || vp_w != self.last_vp_w
                || vp_h != self.last_vp_h;

            let user_busy = dragging || move_mask != 0;
            let need_expand = if self.framed_radius <= 0.0 {
                true
            } else {
                bounds_radius > self.framed_radius * Self::FRAME_GROWTH_EPS
            };

            if !user_busy && (!self.framed_once || aspect_changed || need_expand) {
                let fovy = 60.0f32.to_radians();
                orbit_frame_sphere(&mut self.orbit, bounds_center, bounds_radius, fovy, aspect, 1.15);

                self.framed_radius = self.framed_radius.max(bounds_radius);
                self.framed_once = true;

                Self::sync_rig_with_floor_lift(&mut self.orbit, &mut self.rig);
            }

            self.last_bounds_center = bounds_center;
            self.last_bounds_radius = bounds_radius;

            self.last_aspect = aspect;
            self.last_vp_w = vp_w;
            self.last_vp_h = vp_h;

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

            let lit = ensure_lit_pipeline(&mut self.lit, &mut **r)?;

            // Grid stays on y=0 always (world floor), independent from orbit.target.y
            if bounds_radius.is_finite() {
                let g = ensure_grid(&mut self.grid, &mut **r, lit.bgl)?;
                let spacing = {
                    let d = self.orbit.distance.max(0.01);
                    let base = (d * 0.08).max(0.05);
                    let pow10 = 10.0f32.powf(base.log10().floor());
                    pow10.clamp(0.05, 1000.0)
                };

                let cx = (self.orbit.target.x / spacing).round() * spacing;
                let cz = (self.orbit.target.z / spacing).round() * spacing;

                let grid_model = Mat4::from_scale_rotation_translation(
                    Vec3::new(spacing, 1.0, spacing),
                    Quat::IDENTITY,
                    Vec3::new(cx, 0.0, cz),
                );

                Self::write_mat4_ubo(&mut **r, lit.ubo, viewproj * grid_model)?;

                r.set_pipeline(g.pipeline)?;
                r.set_bind_group(0, lit.bg)?;
                r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
                r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
            }

            {
                let world = scene.world();
                let reg_lock = self.scene_bridge.primitives();
                let reg = reg_lock.read();

                for (_id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
                    let gpu = ensure_primitive_gpu(&reg, prim.id, &mut self.prim_cache, &mut **r)?;

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