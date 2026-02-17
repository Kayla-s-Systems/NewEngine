#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{auto_near_far_from_sphere, orbit_frame_sphere, CameraInput, Perspective, Projection};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, BufferSlice, Extent2D, IndexFormat, RectI32, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_materials::api::MaterialRegistryApi;
use newengine_math::{Mat4, Quat, Vec3};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

use newengine_primitives::Primitive;
use newengine_scene::{scene_bounds_cached, update_scene_world};
use newengine_transform::GlobalTransform;

use super::controller::EditorRenderController;
use super::gpu::{ensure_grid, ensure_lit_pipeline, ensure_primitive_gpu, GridMeshParams};

impl EditorRenderController {
    // Editor "floor" constraint: camera must stay above this Y.
    const MIN_CAMERA_Y: f32 = 0.10;

    // Blender-like orbit constraints.
    const MAX_PITCH_ABS: f32 = 1.5184364; // ~87 deg
    const MIN_DISTANCE: f32 = 0.30;

    // NOTE: camera framing is explicit (hotkey/button) + startup/aspect changes.
    // Auto-framing on scene growth is intentionally disabled to keep the world reference
    // stable while transforming/animating objects (prevents the grid "moving with the object").

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
    fn write_lit_ubo(
        r: &mut dyn newengine_core::render::RenderApi,
        ubo: newengine_core::render::BufferId,
        m: Mat4,
        base_color: [f32; 4],
    ) -> EngineResult<()> {
        let cols = m.to_cols_array();
        // std140: mat4 (64) + vec4 (16)
        let mut bytes: [u8; 80] = [0u8; 80];
        for (i, f) in cols.iter().enumerate() {
            let off = i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }
        let base_off = 64;
        for i in 0..4 {
            let off = base_off + i * 4;
            bytes[off..off + 4].copy_from_slice(&base_color[i].to_ne_bytes());
        }
        r.write_buffer(ubo, 0, &bytes)
    }

    #[inline]
    fn pick_entity(
        viewproj: Mat4,
        vp_w: u32,
        vp_h: u32,
        x_px: f32,
        y_px: f32,
        world: &newengine_ecs::World,
    ) -> Option<newengine_ecs::EntityId> {
        if vp_w == 0 || vp_h == 0 {
            return None;
        }

        let inv = viewproj.inverse();

        // NDC: x in [-1,1], y in [-1,1] (top-left origin in pixels).
        let x = ((x_px + 0.5) / vp_w as f32) * 2.0 - 1.0;
        let y = 1.0 - ((y_px + 0.5) / vp_h as f32) * 2.0;

        let near = inv * newengine_math::Vec4::new(x, y, 0.0, 1.0);
        let far = inv * newengine_math::Vec4::new(x, y, 1.0, 1.0);

        let near3 = near.truncate() / near.w.max(1e-6);
        let far3 = far.truncate() / far.w.max(1e-6);

        let ray_o: Vec3 = near3;
        let mut ray_d: Vec3 = far3 - near3;
        let len2 = ray_d.length_squared();
        if len2 <= 1e-12 {
            return None;
        }
        ray_d *= len2.sqrt().recip();

        // Best-effort bounds: sphere from matrix scale.
        let mut best_t = f32::INFINITY;
        let mut best_e: Option<newengine_ecs::EntityId> = None;

        for (e, _prim, gt) in world.query2::<Primitive, GlobalTransform>() {
            let m = gt.0;
            let center = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);

            let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
            let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
            let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
            let r = 0.8660254 * sx.max(sy).max(sz).max(1e-3);

            // Ray-sphere intersection.
            let oc = ray_o - center;
            let b = oc.dot(ray_d);
            let c = oc.length_squared() - r * r;
            let disc = b * b - c;
            if disc < 0.0 {
                continue;
            }
            let t = -b - disc.sqrt();
            if t > 0.0 && t < best_t {
                best_t = t;
                best_e = Some(e);
            }
        }

        best_e
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

            let (dx_px, dy_px, wheel_y, _hovered, look_drag, pan_drag, ui_busy) =
                self.viewport_bridge.read_orbit_input();
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
                let b = scene_bounds_cached(world);
                if let Some(s) = b.sphere {
                    (s.center, s.radius.max(0.001))
                } else {
                    self.default_bounds()
                }
            };

            let base_speed = (bounds_radius.max(0.01) * 2.0).clamp(0.5, 200.0);
            Self::apply_move_axes(&mut self.orbit, move_mask, dt, base_speed);

            let mut move_axis = Vec3::ZERO;
            if pan_drag {
                // Pan in camera plane: pixels -> normalized units.
                // Scale is tuned inside OrbitController via pan_speed * distance.
                move_axis.x = -dx_px;
                move_axis.y = dy_px;
            }

            let input = CameraInput {
                look_active: look_drag,
                look_delta: newengine_math::Vec2::new(dx_px, -dy_px),
                move_axis,
                speed_mul: 1.0,
                zoom_delta: wheel_y,
            };

            // Viewport aspect (vp_h > 0 in this branch).
            let aspect = vp_w as f32 / (vp_h as f32);

            self.orbit.look_sens = 0.0045;
            self.orbit.dolly_speed = (bounds_radius * 0.25).clamp(0.05, 10.0);
            self.orbit.pan_speed = (bounds_radius * 0.0025).clamp(0.001, 1.0);

            // Apply controller then enforce floor by lifting pivot (Blender-like).
            Self::enforce_orbit_basic(&mut self.orbit);
            self.orbit.apply(&mut self.rig, input, dt);
            Self::sync_rig_with_floor_lift(&mut self.orbit, &mut self.rig);

            // Framing:
            // We keep framing strictly explicit (hotkey F / button) plus a single startup frame.
            // Auto-framing on aspect/bounds changes is hostile to editing: during rotate/scale the
            // scene bounds (often AABB-derived) can change every frame, which makes the orbit pivot
            // chase the selection and looks like the grid/camera moves together with the object.

            // "Busy" means the user is actively controlling the camera OR manipulating scene objects.
            // We must not auto-frame while the gizmo is being dragged, otherwise orbit.target will
            // chase the changing scene bounds and the world grid will look like it's moving.
            //
            // IMPORTANT: Even when the user releases the mouse, the scene bounds may keep changing
            // for a frame (e.g. object rotation changes an AABB-derived bounds radius). Treat such
            // changes as "busy" as well, otherwise the camera pivot will "chase" the selection and
            // it will look like the grid/camera moves together with the object during rotate.
            let bounds_center_delta = (bounds_center - self.last_bounds_center).length();
            let bounds_radius_delta = (bounds_radius - self.last_bounds_radius).abs();
            let bounds_changed = bounds_center_delta > (bounds_radius.max(0.001) * 0.0005)
                || bounds_radius_delta > (bounds_radius.max(0.001) * 0.0005);

            let user_busy = look_drag || pan_drag || move_mask != 0 || ui_busy || bounds_changed;
            // UI-driven frame request (hotkey F / button).
            let frame_seq = self.viewport_bridge.read_frame_request();
            let explicit_frame = frame_seq != self.last_frame_seq;
            if explicit_frame {
                self.last_frame_seq = frame_seq;
            }

            if explicit_frame || (!self.framed_once && !user_busy) {
                let fovy = 60.0f32.to_radians();
                orbit_frame_sphere(
                    &mut self.orbit,
                    bounds_center,
                    bounds_radius,
                    fovy,
                    aspect,
                    1.15,
                );

                self.framed_radius = bounds_radius;
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

            // Make camera matrices available to the UI for overlays (selection highlight, gizmos).
            self.viewport_bridge
                .publish_camera_frame(view, proj, vp_w, vp_h);

            // Selection picking: UI requests a pick with a cursor position.
            let (pick_seq, pick_x, pick_y) = self.viewport_bridge.read_pick_request();
            if pick_seq != self.last_pick_seq {
                self.last_pick_seq = pick_seq;

                let world = scene.world();
                let picked = Self::pick_entity(viewproj, vp_w, vp_h, pick_x, pick_y, world);
                self.scene_bridge.set_selection(picked);
            }

            let grid_settings = self.scene_bridge.grid_settings();

            r.begin_render_target(
                BeginRenderTargetDesc::new(rt)
                    .with_clear_depth(1.0)
                    .with_clear_color(grid_settings.background_color),
            )?;
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

            let lit = ensure_lit_pipeline(&mut self.lit, &mut **r)?;

            // Grid stays on y=0 always (world floor), independent from orbit.target.y
            if bounds_radius.is_finite() {
                let g = ensure_grid(
                    &mut self.grid,
                    &mut **r,
                    lit.bgl,
                    GridMeshParams {
                        half_lines: grid_settings.half_lines as i32,
                        major_every: grid_settings.major_every as i32,
                        minor_color: grid_settings.minor_color,
                        major_color: grid_settings.major_color,
                    },
                )?;
                let spacing = grid_settings.effective_spacing(self.orbit.distance);

                // IMPORTANT (Editor UX): the grid is a world-space reference plane.
                // It must NOT follow selection/orbit target, otherwise transforming objects
                // makes the grid appear to "move with the object".
                // If you ever want an "infinite grid" variant, expose it as an explicit toggle.
                let (cx, cz) = if grid_settings.follow_camera {
                    (
                        (self.orbit.target.x / spacing).round() * spacing,
                        (self.orbit.target.z / spacing).round() * spacing,
                    )
                } else {
                    (0.0, 0.0)
                };

                let grid_model = Mat4::from_scale_rotation_translation(
                    Vec3::new(spacing, 1.0, spacing),
                    Quat::IDENTITY,
                    Vec3::new(cx, 0.0, cz),
                );

                // Grid uses its own vertex colors; base_color is irrelevant but kept defined.
                Self::write_lit_ubo(&mut **r, lit.grid_ubo, viewproj * grid_model, [1.0, 1.0, 1.0, 1.0])?;

                r.set_pipeline(g.pipeline)?;
                r.set_bind_group(0, lit.grid_bg)?;
                r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
                r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
            }

            {
                let world = scene.world();
                let reg_lock = self.scene_bridge.primitives();
                let reg = reg_lock.read();
                let mats_lock = self.scene_bridge.materials();
                let mats = mats_lock.read();

                for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
                    let gpu = ensure_primitive_gpu(&reg, prim.id, &mut self.prim_cache, &mut **r)?;

                    let mvp = viewproj * gt.0;

                    // Material-driven base color (fallback to primitive color).
                    let base_color = world
                        .get::<newengine_materials::MaterialRef>(id)
                        .and_then(|mr| mats.get(mr.id))
                        .map(|d| d.base_color)
                        .unwrap_or(prim.color);

                    Self::write_lit_ubo(&mut **r, lit.ubo, mvp, base_color)?;

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