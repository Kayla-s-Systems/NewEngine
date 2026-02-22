#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{auto_near_far_from_sphere, orbit_frame_sphere, CameraInput, Perspective, Projection};
use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, BufferSlice, Extent2D, IndexFormat, RectI32, Viewport,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_math::{Mat4, Quat, Vec2, Vec3};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_primitives::builtins as prim_builtins;
use newengine_primitives::Primitive;
use newengine_scene::{scene_bounds_cached, update_scene_world};
use newengine_sim::CameraRigComp;
use newengine_transform::GlobalTransform;

use super::controller::{EditorRenderController, PerDrawUbo};
use super::gpu::{ensure_grid, ensure_lit_pipeline, ensure_primitive_gpu, GridMeshParams, LIT_UBO_SIZE};
use crate::editor_camera::{EditorCameraController, EditorCameraMode};

#[inline]
fn quat_from_forward_z(dir_ws: Vec3) -> Quat {
    let fwd = Vec3::Z;
    let d = dir_ws.normalize_or_zero();
    if d.length_squared() <= 1e-8 {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(fwd, d)
}

const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, Default)]
struct PackedLights {
    ambient: [f32; 4],
    dir_dir_intensity: [f32; 4],
    dir_color: [f32; 4],
    point_pos_range: [[f32; 4]; MAX_POINT_LIGHTS],
    point_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
    point_count_pad: [f32; 4],
}

impl PackedLights {
    const UBO_SIZE: usize = 336;

    #[inline]
    fn from_world(world: &newengine_ecs::World) -> Self {
        let amb = world.resource::<AmbientLight>().copied().unwrap_or_default();
        let ambient = [amb.color[0], amb.color[1], amb.color[2], amb.intensity];

        let mut best_dir: Option<(u64, DirectionalLight)> = None;
        for (e, l) in world.query::<DirectionalLight>() {
            let k = e.stable_u64();
            if best_dir.map(|(bk, _)| k < bk).unwrap_or(true) {
                best_dir = Some((k, *l));
            }
        }
        let dir = best_dir.map(|(_, l)| l).unwrap_or_default();
        let dir_dir_intensity = [dir.direction_ws[0], dir.direction_ws[1], dir.direction_ws[2], dir.intensity];
        let dir_color = [dir.color[0], dir.color[1], dir.color[2], 0.0];

        let mut pts: Vec<(u64, [f32; 4], [f32; 4])> = Vec::new();
        for (e, pl, gt) in world.query2::<PointLight, GlobalTransform>() {
            let m = gt.0;
            let pos = [m.w_axis.x, m.w_axis.y, m.w_axis.z, pl.range.max(1e-3)];
            let col = [pl.color[0], pl.color[1], pl.color[2], pl.intensity.max(0.0)];
            pts.push((e.stable_u64(), pos, col));
        }
        pts.sort_by(|a, b| a.0.cmp(&b.0));

        if pts.len() > MAX_POINT_LIGHTS {
            log::warn!(
                "render: point lights truncated: requested={} max={} (deterministic keep=min stable id)",
                pts.len(),
                MAX_POINT_LIGHTS
            );
        }

        let mut out = Self {
            ambient,
            dir_dir_intensity,
            dir_color,
            ..Self::default()
        };

        let n = pts.len().min(MAX_POINT_LIGHTS);
        for i in 0..n {
            out.point_pos_range[i] = pts[i].1;
            out.point_color_intensity[i] = pts[i].2;
        }
        out.point_count_pad = [n as f32, 0.0, 0.0, 0.0];

        out
    }

    #[inline]
    fn write_into(&self, bytes: &mut [u8; Self::UBO_SIZE]) {
        let mut off = 144;

        fn write_vec4(dst: &mut [u8], off: &mut usize, v: [f32; 4]) {
            for i in 0..4 {
                let o = *off + i * 4;
                dst[o..o + 4].copy_from_slice(&v[i].to_ne_bytes());
            }
            *off += 16;
        }

        write_vec4(bytes, &mut off, self.ambient);
        write_vec4(bytes, &mut off, self.dir_dir_intensity);
        write_vec4(bytes, &mut off, self.dir_color);
        for i in 0..MAX_POINT_LIGHTS {
            write_vec4(bytes, &mut off, self.point_pos_range[i]);
            write_vec4(bytes, &mut off, self.point_color_intensity[i]);
        }
        write_vec4(bytes, &mut off, self.point_count_pad);
    }
}

impl EditorRenderController {
    const MIN_CAMERA_Y: f32 = 0.10;

    const MAX_PITCH_ABS: f32 = 1.5184364;
    const MIN_DISTANCE: f32 = 0.30;

    const GRID_HALF_LINES: i32 = 80;
    const GRID_MAJOR_EVERY: i32 = 10;
    const GRID_MINOR_COLOR: [f32; 4] = [0.32, 0.32, 0.34, 1.0];
    const GRID_MAJOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
    const GRID_BACKGROUND_COLOR: [f32; 4] = [0.10, 0.10, 0.11, 1.0];

    #[inline]
    fn enforce_orbit_basic(orbit: &mut newengine_camera::OrbitController) {
        orbit.distance = orbit.distance.max(Self::MIN_DISTANCE);
        orbit.pitch_limit = orbit.pitch_limit.min(Self::MAX_PITCH_ABS);
        orbit.pitch = orbit.pitch.clamp(-Self::MAX_PITCH_ABS, Self::MAX_PITCH_ABS);
    }

    #[inline]
    fn grid_spacing(camera_distance: f32) -> f32 {
        let d = camera_distance.max(0.01);
        let base = (d * 0.08).max(0.05);
        let pow10 = 10.0f32.powf(base.log10().floor());
        pow10.clamp(0.05, 1000.0)
    }

    #[inline]
    fn compute_rot(orbit: &newengine_camera::OrbitController) -> Quat {
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

        let rot = Self::compute_rot(orbit);
        let mut pos = orbit.target + (rot * Vec3::Z) * orbit.distance;
        if pos.y < Self::MIN_CAMERA_Y {
            pos.y = Self::MIN_CAMERA_Y;
        }
        rig.position = pos;
        rig.rotation = rot;
    }

    #[inline]
    fn write_lit_ubo(
        r: &mut dyn newengine_core::render::RenderApi,
        ubo: newengine_core::render::BufferId,
        mvp: Mat4,
        model: Mat4,
        base_color: [f32; 4],
        lights: &PackedLights,
    ) -> EngineResult<()> {
        let mut bytes: [u8; PackedLights::UBO_SIZE] = [0u8; PackedLights::UBO_SIZE];

        let mvp_cols = mvp.to_cols_array();
        for (i, f) in mvp_cols.iter().enumerate() {
            let off = i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }

        let model_cols = model.to_cols_array();
        let model_off = 64;
        for (i, f) in model_cols.iter().enumerate() {
            let off = model_off + i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }

        let base_off = 128;
        for i in 0..4 {
            let off = base_off + i * 4;
            bytes[off..off + 4].copy_from_slice(&base_color[i].to_ne_bytes());
        }

        lights.write_into(&mut bytes);

        r.write_buffer(ubo, 0, &bytes)
    }

    #[inline]
    fn collect_lights(world: &newengine_ecs::World) -> PackedLights {
        PackedLights::from_world(world)
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

        let mut best_t = f32::INFINITY;
        let mut best_e: Option<newengine_ecs::EntityId> = None;

        for (e, _prim, gt) in world.query2::<Primitive, GlobalTransform>() {
            let m = gt.0;
            let center = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);

            let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
            let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
            let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
            let r = 0.8660254 * sx.max(sy).max(sz).max(1e-3);

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

    #[inline]
    fn ensure_per_draw_ubo(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: super::gpu::LitPipeline,
        key: u64,
    ) -> EngineResult<PerDrawUbo> {
        if let Some(e) = self.per_draw_ubo.get(&key).copied() {
            return Ok(e);
        }

        let ubo = r.create_buffer(
            newengine_core::render::BufferDesc::new(
                LIT_UBO_SIZE,
                newengine_core::render::BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
                .with_label("editor_lit_entity_ubo"),
        )?;

        let bg = r.create_bind_group(
            newengine_core::render::BindGroupDesc::new(lit.bgl)
                .with_label("editor_lit_entity_bg")
                .with_uniform0(newengine_core::render::BufferBinding::new(ubo, 0, LIT_UBO_SIZE)),
        )?;

        let entry = PerDrawUbo {
            ubo,
            bg,
            last_seen_frame: self.frame_index,
        };
        self.per_draw_ubo.insert(key, entry);
        Ok(entry)
    }

    fn gc_per_draw_ubos(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        let now = self.frame_index;
        let grace = 2_u64;

        let mut dead: Vec<u64> = Vec::new();
        for (k, v) in &self.per_draw_ubo {
            if now.saturating_sub(v.last_seen_frame) > grace {
                dead.push(*k);
            }
        }
        for k in dead {
            if let Some(v) = self.per_draw_ubo.remove(&k) {
                r.destroy_bind_group(v.bg);
                r.destroy_buffer(v.ubo);
            }
        }
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

        self.frame_index = self.frame_index.saturating_add(1).max(1);

        let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);
        {
            let mut p = self.previews.lock();
            p.pump(&mut **r, dt)?;
        }

        if vp_w > 0 && vp_h > 0 {
            let extent = Extent2D::new(vp_w, vp_h);
            let rt = self.ensure_viewport_rt(&mut **r, extent)?;

            let (dx_px, dy_px, wheel_y, _hovered, look_drag, pan_drag, ui_busy, fly_rmb) =
                self.viewport_bridge.read_camera_input();

            let move_mask = self.viewport_bridge.read_move_keys();

            // Single source of truth for move bits.
            const MOVE_W: u64 = 1 << 0;
            const MOVE_A: u64 = 1 << 1;
            const MOVE_S: u64 = 1 << 2;
            const MOVE_D: u64 = 1 << 3;
            const MOVE_UP: u64 = 1 << 4;
            const MOVE_DOWN: u64 = 1 << 5;
            const MOVE_SHIFT: u64 = 1 << 6;

            let shift = (move_mask & MOVE_SHIFT) != 0;

            let aspect = if vp_h > 0 {
                (vp_w as f32 / vp_h as f32).max(1e-6)
            } else {
                1.0
            };

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

            // Resolve active camera controller state from ECS (no renderer-owned controller state).
            let cam_id = scene.active_camera().unwrap_or_else(|| scene.root().unwrap_or_default());
            let world_mut = scene.world_mut();

            let mut ctrl = world_mut
                .get::<EditorCameraController>(cam_id)
                .copied()
                .unwrap_or_default();

            // Mode hint from UI: RMB-held capture means "editor fly".
            // When not captured, fall back to orbit for safe UI interaction.
            let prev_mode = ctrl.mode;
            let desired_mode = if fly_rmb {
                EditorCameraMode::Fly
            } else {
                EditorCameraMode::Orbit
            };

            let mut rig = world_mut
                .get::<CameraRigComp>(cam_id)
                .copied()
                .map(|c| c.0)
                .unwrap_or_default();

            if prev_mode != desired_mode {
                match (prev_mode, desired_mode) {
                    (EditorCameraMode::Fly, EditorCameraMode::Orbit) => {
                        // Preserve world position: sync orbit target/yaw/pitch from current rig.
                        ctrl.sync_orbit_from_rig(&rig);
                    }
                    (EditorCameraMode::Orbit, EditorCameraMode::Fly) => {
                        // Avoid first-frame rotation snap when entering Fly.
                        ctrl.sync_fly_from_rig(&rig);
                    }
                    _ => {}
                }
            }

            ctrl.mode = desired_mode;

            // Deterministic decode.
            let fwd = ((move_mask & MOVE_W) != 0) as i32 - ((move_mask & MOVE_S) != 0) as i32;
            let right = ((move_mask & MOVE_D) != 0) as i32 - ((move_mask & MOVE_A) != 0) as i32;
            let up = ((move_mask & MOVE_UP) != 0) as i32 - ((move_mask & MOVE_DOWN) != 0) as i32;

            let mut move_axis = Vec3::ZERO;
            let mut speed_mul = if shift { 2.0 } else { 1.0 };

            // Orbit-only pan drag.
            if pan_drag && ctrl.mode == EditorCameraMode::Orbit {
                move_axis.x = -dx_px;
                move_axis.y = dy_px;
            }

            if ctrl.mode == EditorCameraMode::Fly {
                move_axis.x = right as f32;
                move_axis.y = up as f32;
                move_axis.z = fwd as f32;
            } else {
                move_axis.x += (right as f32) * base_speed;
                move_axis.z += (fwd as f32) * base_speed;
            }

            let input = CameraInput {
                look_active: look_drag,
                look_delta: Vec2::new(dx_px, -dy_px),
                move_axis,
                speed_mul,
                zoom_delta: wheel_y,
            };

            if ctrl.mode == EditorCameraMode::Orbit {
                ctrl.orbit.look_sens = 0.0045;
                ctrl.orbit.dolly_speed = (bounds_radius * 0.25).clamp(0.05, 10.0);
                ctrl.orbit.pan_speed = (bounds_radius * 0.0025).clamp(0.001, 1.0);

                Self::enforce_orbit_basic(&mut ctrl.orbit);
                ctrl.apply(&mut rig, input, dt);
                Self::sync_rig_with_floor_lift(&mut ctrl.orbit, &mut rig);
            } else {
                ctrl.fly.look_sens = 0.0045;
                ctrl.fly.move_speed = (bounds_radius * 0.75).clamp(0.5, 200.0);
                ctrl.apply(&mut rig, input, dt);
            }

            let _ = world_mut.insert(cam_id, ctrl);
            let _ = world_mut.insert(cam_id, CameraRigComp(rig));

            // Framing
            let bounds_center_delta = (bounds_center - self.last_bounds_center).length();
            let bounds_radius_delta = (bounds_radius - self.last_bounds_radius).abs();
            let bounds_changed = bounds_center_delta > (bounds_radius.max(0.001) * 0.0005)
                || bounds_radius_delta > (bounds_radius.max(0.001) * 0.0005);

            let user_busy = look_drag || pan_drag || move_mask != 0 || ui_busy || bounds_changed;
            let frame_seq = self.viewport_bridge.read_frame_request();
            let frame_all = self.viewport_bridge.read_frame_all();
            let explicit_frame = frame_seq != self.last_frame_seq;
            if explicit_frame {
                self.last_frame_seq = frame_seq;
            }

            if explicit_frame || (!self.framed_once && !user_busy) {
                let (fc, fr) = if explicit_frame && !frame_all {
                    let sel = self.scene_bridge.selection();
                    if let Some(e) = sel {
                        let world = scene.world();
                        if let Some(b) = newengine_scene::selection_world_bounds(world, [e].into_iter()) {
                            let c = b.center();
                            let r = b.half_extents().length().max(0.001);
                            (c, r)
                        } else {
                            (bounds_center, bounds_radius)
                        }
                    } else {
                        (bounds_center, bounds_radius)
                    }
                } else {
                    (bounds_center, bounds_radius)
                };

                let fovy = 60.0f32.to_radians();
                orbit_frame_sphere(&mut ctrl.orbit, fc, fr, fovy, aspect, 1.15);

                self.framed_radius = fr;
                self.framed_once = true;

                Self::sync_rig_with_floor_lift(&mut ctrl.orbit, &mut rig);
            }

            self.last_bounds_center = bounds_center;
            self.last_bounds_radius = bounds_radius;

            self.last_aspect = aspect;
            self.last_vp_w = vp_w;
            self.last_vp_h = vp_h;

            let (near, far) = auto_near_far_from_sphere(ctrl.orbit.distance, bounds_radius);
            self.projection = Projection::Perspective(Perspective::new(
                60.0f32.to_radians(),
                aspect,
                near,
                far,
            ));

            let proj = self.projection.matrix();
            let view = rig.view_matrix();
            let viewproj = proj * view;

            let inv_view = view.inverse();
            let cam_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);
            let cam_fwd = -Vec3::new(inv_view.z_axis.x, inv_view.z_axis.y, inv_view.z_axis.z);
            self.viewport_bridge.publish_camera_spawn(cam_pos, cam_fwd);

            self.viewport_bridge.publish_camera_frame(view, proj, vp_w, vp_h);

            let (pick_seq, pick_x, pick_y) = self.viewport_bridge.read_pick_request();
            if pick_seq != self.last_pick_seq {
                self.last_pick_seq = pick_seq;

                let world = scene.world();
                let picked = Self::pick_entity(viewproj, vp_w, vp_h, pick_x, pick_y, world);
                self.scene_bridge.set_selection(picked);
            }

            r.begin_render_target(
                BeginRenderTargetDesc::new(rt)
                    .with_clear_depth(1.0)
                    .with_clear_color(Self::GRID_BACKGROUND_COLOR),
            )?;
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

            let lit = ensure_lit_pipeline(&mut self.lit, &mut **r)?;
            let world_lights = Self::collect_lights(scene.world());

            if bounds_radius.is_finite() {
                let g = ensure_grid(
                    &mut self.grid,
                    &mut **r,
                    lit.bgl,
                    GridMeshParams {
                        half_lines: Self::GRID_HALF_LINES,
                        major_every: Self::GRID_MAJOR_EVERY,
                        minor_color: Self::GRID_MINOR_COLOR,
                        major_color: Self::GRID_MAJOR_COLOR,
                    },
                )?;
                let spacing = Self::grid_spacing(ctrl.orbit.distance);

                let grid_model = Mat4::from_scale_rotation_translation(
                    Vec3::new(spacing, 1.0, spacing),
                    Quat::IDENTITY,
                    Vec3::ZERO,
                );

                Self::write_lit_ubo(
                    &mut **r,
                    lit.grid_ubo,
                    viewproj * grid_model,
                    grid_model,
                    [1.0, 1.0, 1.0, 1.0],
                    &world_lights,
                )?;

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

                    let model = gt.0;
                    let mvp = viewproj * model;

                    let base_color = world
                        .get::<newengine_materials::MaterialRef>(id)
                        .and_then(|mr| mats.get(mr.id))
                        .map(|d| d.base_color)
                        .unwrap_or([1.0, 0.0, 1.0, 1.0]);

                    let key = id.stable_u64();
                    let mut per = self.ensure_per_draw_ubo(&mut **r, lit, key)?;
                    per.last_seen_frame = self.frame_index;
                    self.per_draw_ubo.insert(key, per);

                    Self::write_lit_ubo(&mut **r, per.ubo, mvp, model, base_color, &world_lights)?;

                    r.set_pipeline(lit.pipeline)?;
                    r.set_bind_group(0, per.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(gpu.index_count))?;
                }
            }

            {
                let world = scene.world();
                let reg_lock = self.scene_bridge.primitives();
                let reg = reg_lock.read();

                let sphere_id = prim_builtins::ID_SPHERE_UV;
                let sphere_gpu = ensure_primitive_gpu(&reg, sphere_id, &mut self.prim_cache, &mut **r)?;

                let cone_id = prim_builtins::ID_CONE;
                let cone_gpu = ensure_primitive_gpu(&reg, cone_id, &mut self.prim_cache, &mut **r)?;

                let mut dirs: Vec<(u64, DirectionalLight, Mat4)> = Vec::new();
                for (e, l, gt) in world.query2::<DirectionalLight, GlobalTransform>() {
                    dirs.push((e.stable_u64(), *l, gt.0));
                }
                dirs.sort_by(|a, b| a.0.cmp(&b.0));

                for (k, dl, m) in dirs {
                    let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
                    let dir = Vec3::new(dl.direction_ws[0], dl.direction_ws[1], dl.direction_ws[2]).normalize_or_zero();

                    let rot = quat_from_forward_z(dir);
                    let scale = Vec3::splat(0.35);
                    let model = Mat4::from_scale_rotation_translation(scale, rot, pos);
                    let mvp = viewproj * model;

                    let base_color = [1.0, 0.95, 0.35, 1.0];
                    let mut per = self.ensure_per_draw_ubo(&mut **r, lit, k)?;
                    per.last_seen_frame = self.frame_index;
                    self.per_draw_ubo.insert(k, per);
                    Self::write_lit_ubo(&mut **r, per.ubo, mvp, model, base_color, &world_lights)?;

                    r.set_pipeline(lit.pipeline)?;
                    r.set_bind_group(0, per.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(cone_gpu.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(cone_gpu.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(cone_gpu.index_count))?;

                    let line_len = 1.2_f32;
                    let line_pos = pos + dir * (line_len * 0.5);
                    let line_scale = Vec3::new(0.08, 0.08, line_len);
                    let line_model = Mat4::from_scale_rotation_translation(line_scale, rot, line_pos);
                    let line_mvp = viewproj * line_model;
                    let line_color = [1.0, 0.85, 0.25, 1.0];
                    let line_key = k ^ 0xD1A1_0000_0000_0000u64;
                    let mut per2 = self.ensure_per_draw_ubo(&mut **r, lit, line_key)?;
                    per2.last_seen_frame = self.frame_index;
                    self.per_draw_ubo.insert(line_key, per2);
                    Self::write_lit_ubo(&mut **r, per2.ubo, line_mvp, line_model, line_color, &world_lights)?;

                    r.set_pipeline(lit.pipeline)?;
                    r.set_bind_group(0, per2.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(cone_gpu.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(cone_gpu.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(cone_gpu.index_count))?;
                }

                let mut pts: Vec<(u64, Mat4)> = Vec::new();
                for (e, _pl, gt) in world.query2::<PointLight, GlobalTransform>() {
                    pts.push((e.stable_u64(), gt.0));
                }
                pts.sort_by(|a, b| a.0.cmp(&b.0));

                for (k, m) in pts {
                    let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
                    let scale = Vec3::splat(0.18);
                    let model = Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, pos);
                    let mvp = viewproj * model;

                    let base_color = [1.0, 0.75, 0.25, 1.0];
                    let mut per = self.ensure_per_draw_ubo(&mut **r, lit, k)?;
                    per.last_seen_frame = self.frame_index;
                    self.per_draw_ubo.insert(k, per);
                    Self::write_lit_ubo(&mut **r, per.ubo, mvp, model, base_color, &world_lights)?;

                    r.set_pipeline(lit.pipeline)?;
                    r.set_bind_group(0, per.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(sphere_gpu.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(sphere_gpu.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(sphere_gpu.index_count))?;
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

        self.gc_per_draw_ubos(&mut **r);

        r.end_frame()?;
        Ok(())
    }
}