#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    require_render_api, BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupLayoutDesc,
    BindingKind, BufferBinding, BufferDesc, BufferSlice, BufferUsage, DrawIndexedArgs, Extent2D,
    IndexFormat, MemoryHint, PipelineDesc, PrimitiveTopology, RectI32, RenderTargetDesc, ShaderDesc,
    ShaderStage, TextureFormat, VertexAttribute, VertexFormat, VertexLayout, Viewport,
};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;
use newengine_ui::{AssetAccess, AssetServiceClient};

use glam::{Mat4, Vec3};
use newengine_camera::{
    auto_near_far_from_sphere, orbit_frame_sphere, orbit_set_angles, CameraInput, CameraRig,
    OrbitController, Perspective, Projection,
};

use crate::plugin_manager_bridge::PluginManagerBridge;
use crate::viewport_bridge::ViewportBridge;

use newengine_core::plugins::default_host_api;

use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind};

// Orbit controls are driven by UI via `ViewportBridge` to avoid leaking global input state.

#[derive(Clone, Copy)]
struct GridGpu {
    vb: newengine_core::render::BufferId,
    vs: newengine_core::render::ShaderId,
    fs: newengine_core::render::ShaderId,
    pipeline: newengine_core::render::PipelineId,
    vertex_count: u32,
}

#[derive(Clone, Copy)]
struct DemoGpu {
    vb: newengine_core::render::BufferId,
    vs: newengine_core::render::ShaderId,
    fs: newengine_core::render::ShaderId,
    pipeline: newengine_core::render::PipelineId,
}

#[derive(Clone, Copy)]
struct ModelGpu {
    vb: newengine_core::render::BufferId,
    ib: newengine_core::render::BufferId,
    ubo: newengine_core::render::BufferId,

    bgl: newengine_core::render::BindGroupLayoutId,
    bg: newengine_core::render::BindGroupId,

    vs: newengine_core::render::ShaderId,
    fs: newengine_core::render::ShaderId,
    pipeline: newengine_core::render::PipelineId,

    index_count: u32,
}

pub struct EditorRenderController {
    model_center: [f32; 3],
    model_radius: f32,
    model_framed_once: bool,
    clear_color: [f32; 4],
    last_w: u32,
    last_h: u32,
    demo: Option<DemoGpu>,
    grid: Option<GridGpu>,
    grid_params: Option<(f32, f32, f32)>,
    model: Option<ModelGpu>,
    model_loaded_once: bool,

    orbit: OrbitController,
    rig: CameraRig,
    projection: Projection,

    assets: AssetServiceClient,

    viewport_bridge: std::sync::Arc<ViewportBridge>,
    plugins_bridge: std::sync::Arc<PluginManagerBridge>,
    viewport_rt: Option<newengine_core::render::RenderTargetId>,
    viewport_rt_extent: Extent2D,
}

impl EditorRenderController {
    #[inline]
    pub fn new(
        clear_color: [f32; 4],
        viewport_bridge: std::sync::Arc<ViewportBridge>,
        plugins_bridge: std::sync::Arc<PluginManagerBridge>,
    ) -> Self {
        // Engine baseline coordinate system:
        // - right-handed
        // - +Y up
        // - -Z forward
        // CameraRig::forward() points along -Z.
        let mut orbit = OrbitController::default();
        orbit_set_angles(&mut orbit, 0.7853982, 0.55);
        orbit.distance = 4.1;

        let rig = CameraRig::default();
        let projection = Projection::Perspective(Perspective::new(
            60.0f32.to_radians(),
            1.0,
            0.01,
            1000.0,
        ));

        Self {
            model_center: [0.0, 0.0, 0.0],
            model_radius: 1.0,
            model_framed_once: false,
            clear_color,
            last_w: 0,
            last_h: 0,
            demo: None,
            grid: None,
            grid_params: None,
            model: None,
            model_loaded_once: false,

            orbit,
            rig,
            projection,

            assets: AssetServiceClient::new(default_host_api()),

            viewport_bridge,
            plugins_bridge,
            viewport_rt: None,
            viewport_rt_extent: Extent2D::new(0, 0),
        }
    }

    #[inline]
    fn rt_to_ui_tex_user(rt: newengine_core::render::RenderTargetId) -> u64 {
        // Matches `newengine-modules-render-vulkan-ash` convention
        // (renderer/render_target.rs): ui_tex_id = 0x8000_0000 | render_target_id.
        const UI_EXTERNAL_BASE: u32 = 0x8000_0000;
        let id = rt.0.get();
        (UI_EXTERNAL_BASE | (id & 0x7FFF_FFFF)) as u64
    }

    fn ensure_viewport_rt(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        extent: Extent2D,
    ) -> EngineResult<()> {
        if extent.width == 0 || extent.height == 0 {
            if let Some(rt) = self.viewport_rt.take() {
                r.destroy_render_target(rt);
            }
            self.viewport_rt_extent = Extent2D::new(0, 0);
            self.viewport_bridge.publish_tex_user(0);
            return Ok(());
        }

        let need_recreate = match self.viewport_rt {
            None => true,
            Some(_) => self.viewport_rt_extent.width != extent.width
                || self.viewport_rt_extent.height != extent.height,
        };

        if need_recreate {
            if let Some(rt) = self.viewport_rt.take() {
                r.destroy_render_target(rt);
            }

            let rt = r.create_render_target(RenderTargetDesc {
                extent,
                color: TextureFormat::Bgra8Unorm,
                depth: None,
                label: Some("editor.viewport.rt"),
            })?;

            self.viewport_rt = Some(rt);
            self.viewport_rt_extent = extent;

            let tex_user = Self::rt_to_ui_tex_user(rt);
            self.viewport_bridge.publish_tex_user(tex_user);
        }

        Ok(())
    }

    fn load_asset_payload_with_timeout(
        &self,
        logical_path: &str,
        timeout_ms: u64,
    ) -> EngineResult<Option<Vec<u8>>> {
        use newengine_ui::AssetState;

        let id_hex32 = match self.assets.load(logical_path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("asset: load failed path='{logical_path}' err='{e}'");
                return Ok(None);
            }
        };

        let t0 = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            self.assets.pump();

            match self.assets.state(&id_hex32) {
                Ok(AssetState::Ready) => break,
                Ok(AssetState::Failed) => {
                    return Err(EngineError::other(format!(
                        "asset: import failed path='{logical_path}'"
                    )));
                }
                Ok(AssetState::Loading) | Ok(AssetState::Unloaded) => {}
                Err(e) => {
                    return Err(EngineError::other(format!(
                        "asset: state query failed path='{logical_path}' err='{e}'"
                    )));
                }
            }

            if t0.elapsed() >= timeout {
                log::warn!("asset: load timeout path='{logical_path}'");
                return Ok(None);
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let (_meta_json, payload) = self
            .assets
            .blob_wire_v1(&id_hex32)
            .map_err(|e| EngineError::other(format!("asset: blob_wire_v1 failed: {e}")))?;

        Ok(Some(payload))
    }

    fn decode_ne3d_mesh(bytes: &[u8]) -> EngineResult<(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>)> {
        fn need<'a>(bytes: &'a [u8], at: usize, len: usize, what: &str) -> EngineResult<&'a [u8]> {
            let end = at.saturating_add(len);
            if end > bytes.len() {
                return Err(EngineError::other(format!("ne3d: truncated while reading {what}")));
            }
            Ok(&bytes[at..end])
        }

        if bytes.len() < 4 + 4 * 4 {
            return Err(EngineError::other("ne3d: too short"));
        }
        if &bytes[0..4] != b"NE3D" {
            return Err(EngineError::other("ne3d: bad magic"));
        }

        let mut at = 4usize;
        let read_u32 = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);

        let ver = read_u32(need(bytes, at, 4, "version")?);
        at += 4;
        if ver != 1 {
            return Err(EngineError::other(format!("ne3d: unsupported version {ver}")));
        }

        let vtx_count = read_u32(need(bytes, at, 4, "vertex_count")?) as usize;
        at += 4;
        let idx_count = read_u32(need(bytes, at, 4, "index_count")?) as usize;
        at += 4;
        let flags = read_u32(need(bytes, at, 4, "flags")?);
        at += 4;

        let has_normals = (flags & 0x1) != 0;

        let mut pos: Vec<[f32; 3]> = Vec::with_capacity(vtx_count);
        let mut nrm: Vec<[f32; 3]> = Vec::with_capacity(vtx_count);

        for _ in 0..vtx_count {
            let chunk = need(bytes, at, 12, "positions")?;
            at += 12;
            let x = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let y = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let z = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            pos.push([x, y, z]);
        }

        if has_normals {
            for _ in 0..vtx_count {
                let chunk = need(bytes, at, 12, "normals")?;
                at += 12;
                let x = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let y = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                let z = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
                nrm.push([x, y, z]);
            }
        } else {
            nrm.resize(vtx_count, [0.0, 1.0, 0.0]);
        }

        let has_uvs = (flags & 0x2) != 0;
        if has_uvs {
            let uv_bytes = vtx_count
                .checked_mul(8)
                .ok_or_else(|| EngineError::other("ne3d: uv overflow"))?;
            let _ = need(bytes, at, uv_bytes, "uvs")?;
            at += uv_bytes;
        }

        let mut idx: Vec<u32> = Vec::with_capacity(idx_count);
        for _ in 0..idx_count {
            let chunk = need(bytes, at, 4, "indices")?;
            at += 4;
            idx.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }

        Ok((pos, nrm, idx))
    }

    #[inline]
    fn mat4_mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
        let mut o = [0.0f32; 16];
        for c in 0..4 {
            for r in 0..4 {
                o[c * 4 + r] = a[0 * 4 + r] * b[c * 4 + 0]
                    + a[1 * 4 + r] * b[c * 4 + 1]
                    + a[2 * 4 + r] * b[c * 4 + 2]
                    + a[3 * 4 + r] * b[c * 4 + 3];
            }
        }
        o
    }

    #[inline]
    #[inline]
    fn mat4_perspective(fov_y_rad: f32, aspect: f32, z_near: f32, z_far: f32) -> [f32; 16] {
        let f = 1.0 / (0.5 * fov_y_rad).tan();
        let nf = 1.0 / (z_near - z_far);

        // Column-major, RH, Vulkan clip Z: [0..1], with Y flipped (negative).
        // Matrix (row form):
        // [ f/aspect, 0,   0,                     0 ]
        // [ 0,       -f,   0,                     0 ]
        // [ 0,        0,  z_far*nf,  z_far*z_near*nf ]
        // [ 0,        0,  -1,                    0 ]
        //
        // Column-major memory layout:
        // [ m00 m10 m20 m30 | m01 m11 m21 m31 | m02 m12 m22 m32 | m03 m13 m23 m33 ]
        [
            f / aspect, 0.0, 0.0, 0.0, //
            0.0, -f, 0.0, 0.0,        //
            0.0, 0.0, z_far * nf, -1.0, //
            0.0, 0.0, z_far * z_near * nf, 0.0, //
        ]
    }


    #[inline]
    fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    #[inline]
    fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    #[inline]
    fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    #[inline]
    fn vec3_norm(v: [f32; 3]) -> [f32; 3] {
        let l2 = Self::vec3_dot(v, v);
        if l2 <= 0.0 {
            return [0.0, 0.0, 0.0];
        }
        let inv = 1.0 / l2.sqrt();
        [v[0] * inv, v[1] * inv, v[2] * inv]
    }

    #[inline]
    fn mat4_scale_uniform(s: f32) -> [f32; 16] {
        [
            s, 0.0, 0.0, 0.0,
            0.0, s, 0.0, 0.0,
            0.0, 0.0, s, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]
    }


    #[inline]
    fn mat4_look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
        let f = Self::vec3_norm(Self::vec3_sub(center, eye));
        let s = Self::vec3_norm(Self::vec3_cross(f, up));
        let u = Self::vec3_cross(s, f);

        let tx = -Self::vec3_dot(s, eye);
        let ty = -Self::vec3_dot(u, eye);
        let tz = Self::vec3_dot(f, eye);

        // Column-major view matrix:
        // col0 = [ s.x, s.y, s.z, tx ]
        // col1 = [ u.x, u.y, u.z, ty ]
        // col2 = [ -f.x, -f.y, -f.z, tz ]
        // col3 = [ 0, 0, 0, 1 ]
        [
            s[0], s[1], s[2], tx, //
            u[0], u[1], u[2], ty, //
            -f[0], -f[1], -f[2], tz, //
            0.0, 0.0, 0.0, 1.0,
        ]
    }

    #[inline]
    fn mat4_rotation_y(a: f32) -> [f32; 16] {
        let (s, c) = a.sin_cos();
        // Column-major for:
        // [ c, 0,  s, 0 ]
        // [ 0, 1,  0, 0 ]
        // [ -s,0,  c, 0 ]
        // [ 0, 0,  0, 1 ]
        [
            c, 0.0, -s, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            s, 0.0, c, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ]
    }

    fn compile_glsl(
        compiler: &Compiler,
        kind: ShaderKind,
        name: &'static str,
        src: &'static str,
    ) -> EngineResult<Vec<u32>> {
        let mut opts = CompileOptions::new().ok_or_else(|| EngineError::other("shaderc: CompileOptions"))?;
        opts.set_optimization_level(OptimizationLevel::Performance);

        let art = compiler
            .compile_into_spirv(src, kind, name, "main", Some(&opts))
            .map_err(|e| EngineError::other(format!("shaderc: failed to compile {name}: {e}")))?;

        Ok(art.as_binary().to_vec())
    }

    #[inline]
    fn grid_params(radius: f32, camera_distance: f32) -> (f32, f32, f32) {
        // Returns (half_extent, step, major_step).
        // Blender-like behavior:
        // - Grid size follows scene radius, but also stays useful when you zoom far away.
        // - Density stays roughly constant on screen by targeting a fixed number of lines.
        let r = radius.max(0.000_001);
        let d = camera_distance.abs().max(0.000_001);
        let half = (r * 8.0).max(d * 2.0).max(10.0);

        // "Nice" step: 1-2-5 * 10^n.
        let exp = half.log10().floor();
        let base = 10.0f32.powf(exp);
        let mut step = base;
        let target_lines = 120.0f32; // ~ +/-60 lines.
        let raw = (half * 2.0) / target_lines;
        let raw_n = raw / base;
        step = if raw_n <= 1.0 {
            1.0 * base
        } else if raw_n <= 2.0 {
            2.0 * base
        } else if raw_n <= 5.0 {
            5.0 * base
        } else {
            10.0 * base
        };
        step = step.max(0.000_1);

        let major = step * 10.0;
        let half_rounded = ((half / step).ceil() * step).max(step);
        (half_rounded, step, major)
    }

    fn build_grid(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        radius: f32,
        camera_distance: f32,
    ) -> EngineResult<()> {
        let Some(model) = self.model else {
            // Grid is a 3D world overlay; without a camera UBO/bind group it has nothing to bind.
            return Ok(());
        };

        let (half_extent, step, major_step) = Self::grid_params(radius, camera_distance);

        let params = (half_extent, step, major_step);

        // Rebuild only if parameters changed materially.
        let need_rebuild = self.grid.is_none() || self.grid_params.map(|p| p != params).unwrap_or(true);

        if !need_rebuild {
            return Ok(());
        }

        if let Some(g) = self.grid.take() {
            r.destroy_buffer(g.vb);
            r.destroy_pipeline(g.pipeline);
            r.destroy_shader(g.vs);
            r.destroy_shader(g.fs);
        }

        let n = (half_extent / step).round().max(1.0) as i32;
        let major_every = (major_step / step).round().max(1.0) as i32;
        let mut verts: Vec<f32> = Vec::new();
        // Vertex format: pos.xyz, color.rgba (single base color, varying alpha).
        let base_rgb = Vec3::new(0.42, 0.42, 0.44);
        let mut push_v = |p: Vec3, a: f32| {
            let a = a.clamp(0.0, 1.0);
            verts.extend_from_slice(&[p.x, p.y, p.z, base_rgb.x, base_rgb.y, base_rgb.z, a]);
        };

        let y = 0.0f32;

        for i in -n..=n {
            let k = i as f32 * step;

            let is_axis = i == 0;
            let is_major = !is_axis && (i % major_every == 0);

            // Blender-style single hue: major/axis are only denser via alpha.
            let a = if is_axis {
                0.55
            } else if is_major {
                0.35
            } else {
                0.18
            };

            // Lines parallel to X (vary Z).
            push_v(Vec3::new(-half_extent, y, k), a);
            push_v(Vec3::new(half_extent, y, k), a);
            // Lines parallel to Z (vary X).
            push_v(Vec3::new(k, y, -half_extent), a);
            push_v(Vec3::new(k, y, half_extent), a);
        }

        let vbytes = bytemuck::cast_slice::<f32, u8>(&verts);
        let vb = r.create_buffer(
            BufferDesc::new(vbytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
                .with_label("editor_grid_vb"),
        )?;
        r.write_buffer(vb, 0, vbytes)?;

        let compiler = Compiler::new().ok_or_else(|| EngineError::other("shaderc: Compiler"))?;

        const VS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec4 a_col;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
} u;

layout(location = 0) out vec4 v_col;

void main() {
    v_col = a_col;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
"#;

        const FS_SRC: &str = r#"#version 450
	layout(location = 0) in vec4 v_col;
layout(location = 0) out vec4 o_col;

void main() {
    o_col = v_col;
}
"#;

        let vs_spv = Self::compile_glsl(&compiler, ShaderKind::Vertex, "editor_grid.vert", VS_SRC)?;
        let fs_spv = Self::compile_glsl(&compiler, ShaderKind::Fragment, "editor_grid.frag", FS_SRC)?;

        let vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_grid_vs"),
        )?;
        let fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_grid_fs"),
        )?;

        let bgl = model.bgl;

        let layout = VertexLayout::new(
            (7 * std::mem::size_of::<f32>()) as u32,
            vec![
                VertexAttribute::new(0, 0, VertexFormat::Float32x3),
                VertexAttribute::new(
                    1,
                    (3 * std::mem::size_of::<f32>()) as u32,
                    VertexFormat::Float32x4,
                ),
            ],
        );

        let pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
                .with_label("editor_grid_pipeline")
                .with_topology(PrimitiveTopology::LineList)
                .with_vertex_layouts(vec![layout])
                .with_bind_group_layouts(vec![bgl]),
        )?;

        self.grid = Some(GridGpu {
            vb,
            vs,
            fs,
            pipeline,
            vertex_count: (verts.len() / 7) as u32,
        });

        self.grid_params = Some(params);

        Ok(())
    }

    #[inline]
    fn apply_wasd_target_translate(orbit: &mut OrbitController, move_mask: u64, dt: f32, base_speed: f32) {
        if dt <= 0.0 || !dt.is_finite() {
            return;
        }

        let mut forward = 0.0f32;
        let mut right = 0.0f32;
        let mut up = 0.0f32;

        if (move_mask & (1 << 0)) != 0 { forward += 1.0; } // W
        if (move_mask & (1 << 2)) != 0 { forward -= 1.0; } // S
        if (move_mask & (1 << 3)) != 0 { right += 1.0; }   // D
        if (move_mask & (1 << 1)) != 0 { right -= 1.0; }   // A
        if (move_mask & (1 << 5)) != 0 { up += 1.0; }      // E
        if (move_mask & (1 << 4)) != 0 { up -= 1.0; }      // Q

        if forward == 0.0 && right == 0.0 && up == 0.0 {
            return;
        }

        let len_sq = forward * forward + right * right + up * up;
        let inv_len = if len_sq > 1e-6 { len_sq.sqrt().recip() } else { 1.0 };
        forward *= inv_len;
        right *= inv_len;
        up *= inv_len;

        let sprint = if (move_mask & (1 << 6)) != 0 { 4.0 } else { 1.0 };
        let speed = (base_speed * sprint).max(0.0);

        // Move in the horizontal plane aligned to yaw.
        let (sy, cy) = orbit.yaw.sin_cos();
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let rgt = Vec3::new(cy, 0.0, -sy);
        let upv = Vec3::Y;

        let s = speed * dt;
        orbit.target += (fwd * forward + rgt * right + upv * up) * s;
    }

    fn build_demo(&mut self, r: &mut dyn newengine_core::render::RenderApi) -> EngineResult<()> {
        if self.demo.is_some() {
            return Ok(());
        }

        let compiler = Compiler::new().ok_or_else(|| EngineError::other("shaderc: Compiler"))?;

        const VS_SRC: &str = r#"#version 450
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec3 a_col;
layout(location = 0) out vec3 v_col;
void main() {
    v_col = a_col;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

        const FS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 v_col;
layout(location = 0) out vec4 o_col;
void main() {
    o_col = vec4(v_col, 1.0);
}
"#;

        let vs_spv = Self::compile_glsl(&compiler, ShaderKind::Vertex, "editor_demo.vert", VS_SRC)?;
        let fs_spv = Self::compile_glsl(&compiler, ShaderKind::Fragment, "editor_demo.frag", FS_SRC)?;

        let vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_demo_vs"),
        )?;
        let fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_demo_fs"),
        )?;

        let verts: [[f32; 5]; 3] = [
            [-0.70, -0.60, 1.0, 0.2, 0.2],
            [0.70, -0.60, 0.2, 1.0, 0.2],
            [0.00, 0.80, 0.2, 0.4, 1.0],
        ];

        let mut bytes: Vec<u8> = Vec::with_capacity(std::mem::size_of_val(&verts));
        for v in verts {
            for f in v {
                bytes.extend_from_slice(&f.to_ne_bytes());
            }
        }

        let vb = r.create_buffer(
            BufferDesc::new(bytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
                .with_label("editor_demo_vb"),
        )?;
        r.write_buffer(vb, 0, &bytes)?;

        let layout = VertexLayout::new(
            (5 * std::mem::size_of::<f32>()) as u32,
            vec![
                VertexAttribute::new(0, 0, VertexFormat::Float32x2),
                VertexAttribute::new(
                    1,
                    (2 * std::mem::size_of::<f32>()) as u32,
                    VertexFormat::Float32x3,
                ),
            ],
        );

        let pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
                .with_label("editor_demo_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout]),
        )?;

        self.demo = Some(DemoGpu { vb, vs, fs, pipeline });
        Ok(())
    }

    fn build_model(
        &mut self,
        _ctx: &ModuleCtx<'_, impl Send + 'static>,
        r: &mut dyn newengine_core::render::RenderApi,
        target: Extent2D,
    ) -> EngineResult<()> {
        if self.model.is_some() || self.model_loaded_once {
            return Ok(());
        }

        self.model_loaded_once = true;

        // The importer pipeline is responsible for producing engine-native NE3D payload.
        // The editor render controller consumes only that wire format.
        const MODEL_PATH: &str = "models/fox.obj";

        let Some(payload) = self.load_asset_payload_with_timeout(MODEL_PATH, 750)? else {
            log::warn!("model: missing '{MODEL_PATH}'. Place a model under assets/{MODEL_PATH}.");
            return Ok(());
        };

        let (pos, nrm, idx) = Self::decode_ne3d_mesh(&payload)?;
        if pos.is_empty() || idx.is_empty() {
            return Err(EngineError::other("model: empty geometry"));
        }

        let mut bb_min = [f32::INFINITY; 3];
        let mut bb_max = [f32::NEG_INFINITY; 3];
        for p in &pos {
            bb_min[0] = bb_min[0].min(p[0]);
            bb_min[1] = bb_min[1].min(p[1]);
            bb_min[2] = bb_min[2].min(p[2]);
            bb_max[0] = bb_max[0].max(p[0]);
            bb_max[1] = bb_max[1].max(p[1]);
            bb_max[2] = bb_max[2].max(p[2]);
        }

        let center = [
            (bb_min[0] + bb_max[0]) * 0.5,
            (bb_min[1] + bb_max[1]) * 0.5,
            (bb_min[2] + bb_max[2]) * 0.5,
        ];
        // Robust radius: true bounding-sphere radius around AABB center.
        // This avoids cases where a long diagonal extends beyond "half max extent".
        let mut radius = 0.0f32;
        for p in &pos {
            let dx = p[0] - center[0];
            let dy = p[1] - center[1];
            let dz = p[2] - center[2];
            radius = radius.max((dx * dx + dy * dy + dz * dz).sqrt());
        }
        let radius = radius.max(0.001);

        // Persist world bounds for editor AND runtime logic.
        self.model_center = center;
        self.model_radius = radius;

        let stride = 6 * std::mem::size_of::<f32>();
        let mut vbytes: Vec<u8> = Vec::with_capacity(pos.len() * stride);

        for (p, n) in pos.iter().zip(nrm.iter()) {
            vbytes.extend_from_slice(&p[0].to_ne_bytes());
            vbytes.extend_from_slice(&p[1].to_ne_bytes());
            vbytes.extend_from_slice(&p[2].to_ne_bytes());
            vbytes.extend_from_slice(&n[0].to_ne_bytes());
            vbytes.extend_from_slice(&n[1].to_ne_bytes());
            vbytes.extend_from_slice(&n[2].to_ne_bytes());
        }

        let mut ibytes: Vec<u8> = Vec::with_capacity(idx.len() * 4);
        for i in &idx {
            ibytes.extend_from_slice(&i.to_ne_bytes());
        }

        let vb = r.create_buffer(
            BufferDesc::new(vbytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
                .with_label("editor_model_vb"),
        )?;
        r.write_buffer(vb, 0, &vbytes)?;

        let ib = r.create_buffer(
            BufferDesc::new(ibytes.len() as u64, BufferUsage::Index, MemoryHint::CpuToGpu)
                .with_label("editor_model_ib"),
        )?;
        r.write_buffer(ib, 0, &ibytes)?;

        let ubo = r.create_buffer(
            BufferDesc::new(64, BufferUsage::Uniform, MemoryHint::CpuToGpu).with_label("editor_model_ubo"),
        )?;

        let bgl = r.create_bind_group_layout(
            BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer]).with_label("editor_model_bgl"),
        )?;
        let bg = r.create_bind_group(
            BindGroupDesc::new(bgl)
                .with_label("editor_model_bg")
                .with_uniform0(BufferBinding::new(ubo, 0, 64)),
        )?;

        let compiler = Compiler::new().ok_or_else(|| EngineError::other("shaderc: Compiler"))?;

        const VS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
} u;

layout(location = 0) out vec3 v_nrm;

void main() {
    v_nrm = a_nrm;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
"#;

        const FS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 v_nrm;
layout(location = 0) out vec4 o_col;

void main() {
    vec3 n = normalize(v_nrm);
    vec3 l = normalize(vec3(0.35, 0.75, 0.55));
    float ndl = clamp(dot(n, l) * 0.5 + 0.5, 0.0, 1.0);
    o_col = vec4(vec3(ndl), 1.0);
}
"#;

        let vs_spv = Self::compile_glsl(&compiler, ShaderKind::Vertex, "editor_model.vert", VS_SRC)?;
        let fs_spv = Self::compile_glsl(&compiler, ShaderKind::Fragment, "editor_model.frag", FS_SRC)?;

        let vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_model_vs"),
        )?;
        let fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_model_fs"),
        )?;

        let layout = VertexLayout::new(
            stride as u32,
            vec![
                VertexAttribute::new(0, 0, VertexFormat::Float32x3),
                VertexAttribute::new(
                    1,
                    (3 * std::mem::size_of::<f32>()) as u32,
                    VertexFormat::Float32x3,
                ),
            ],
        );

        let pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
                // NOTE: current Vulkan backend uses a single-color render pass.
                // Keep depth disabled until the backend exposes depth targets.
                .with_label("editor_model_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout])
                .with_bind_group_layouts(vec![bgl]),
        )?;

        // Initial contents; real camera MVP is written every frame.
        let mvp = Mat4::IDENTITY.to_cols_array();
        let mut ubytes: Vec<u8> = Vec::with_capacity(64);
        for f in mvp {
            ubytes.extend_from_slice(&f.to_ne_bytes());
        }
        r.write_buffer(ubo, 0, &ubytes)?;

        self.model = Some(ModelGpu {
            vb,
            ib,
            ubo,
            bgl,
            bg,
            vs,
            fs,
            pipeline,
            index_count: idx.len() as u32,
        });

        log::info!(
            "model: loaded '{MODEL_PATH}' vertices={} indices={} radius={:.3}",
            pos.len(),
            idx.len(),
            radius
        );

        Ok(())
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

        // UI publishes desired viewport size (in physical pixels).
        let (vp_w, vp_h) = self.viewport_bridge.read_extent();

        self.build_demo(&mut **r)?;
        if vp_w > 0 && vp_h > 0 {
            self.build_model(ctx, &mut **r, Extent2D::new(vp_w, vp_h))?;
        }

        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;

        // Render the scene into an offscreen render target.
        if vp_w > 0 && vp_h > 0 {
            self.ensure_viewport_rt(&mut **r, Extent2D::new(vp_w, vp_h))?;

            let (dx_px, dy_px, wheel_y, _hovered, dragging) =
                self.viewport_bridge.read_orbit_input();

            let move_mask = self.viewport_bridge.read_move_keys();
            let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);

            // Translate orbit target in world space (editor/game friendly).
            let base_speed = (self.model_radius.max(0.01) * 2.0).clamp(0.5, 200.0);
            Self::apply_wasd_target_translate(&mut self.orbit, move_mask, dt, base_speed);

            // Apply orbit rotation + dolly.
            let input = CameraInput {
                look_active: dragging,
                // Editor convention: drag up pitches camera down.
                look_delta: glam::Vec2::new(dx_px, -dy_px),
                move_axis: Vec3::ZERO,
                speed_mul: 1.0,
                zoom_delta: wheel_y,
            };

            // Match the feel of the previous orbit controller.
            self.orbit.look_sens = 0.0045;
            self.orbit.dolly_speed = 6.0;
            self.orbit.pan_speed = 1.0;

            self.orbit.apply(&mut self.rig, input, dt);

            if let Some(rt) = self.viewport_rt {
                r.begin_render_target(BeginRenderTargetDesc::new(rt))?;

                let extent = Extent2D::new(vp_w, vp_h);
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;


                if let Some(model) = self.model {
                    let aspect = vp_w as f32 / (vp_h.max(1) as f32);

                    let radius = self.model_radius.max(0.000_001);
                    let center = Vec3::new(self.model_center[0], self.model_center[1], self.model_center[2]);

                    // Universal "frame all" — can be invoked by editor OR by game.
                    if !self.model_framed_once {
                        let fovy = 60.0f32.to_radians();
                        orbit_frame_sphere(&mut self.orbit, center, radius, fovy, aspect, 1.15);
                        self.model_framed_once = true;
                    }

                    self.orbit.min_distance = (radius * 0.05).max(0.05);

                    let (near, far) = auto_near_far_from_sphere(self.orbit.distance, radius);

                    // Projection is Vulkan-ready (RH, Y-flip baked, Z 0..1).
                    let fovy = 60.0f32.to_radians();
                    self.projection = Projection::Perspective(Perspective::new(fovy, aspect, near, far));
                    let proj = self.projection.matrix();
                    let view = self.rig.view_matrix();
                    let mvp = proj * view;

                    // Upload MVP for both grid and model.
                    let cols = mvp.to_cols_array();
                    let mut ubytes: [u8; 64] = [0u8; 64];
                    for (i, f) in cols.iter().enumerate() {
                        let off = i * 4;
                        ubytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
                    }
                    r.write_buffer(model.ubo, 0, &ubytes)?;

                    // Grid uses the same camera UBO bind group as the model.
                    self.build_grid(&mut **r, radius, self.orbit.distance)?;
                    if let Some(g) = self.grid {
                        r.set_pipeline(g.pipeline)?;
                        r.set_bind_group(0, model.bg)?;
                        r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
                        r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
                    }

                    r.set_pipeline(model.pipeline)?;
                    r.set_bind_group(0, model.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(model.vb, 0))?;
                    r.set_index_buffer(BufferSlice::new(model.ib, 0), IndexFormat::U32)?;
                    r.draw_indexed(DrawIndexedArgs::new(model.index_count))?;
                } else if let Some(demo) = self.demo {
                    r.set_pipeline(demo.pipeline)?;
                    r.set_vertex_buffer(0, BufferSlice::new(demo.vb, 0))?;
                    r.draw(newengine_core::render::DrawArgs::new(3))?;
                }

                r.end_render_target()?;
                let win_extent = Extent2D::new(w, h);
                r.set_viewport(Viewport::full(win_extent))?;
                r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;
            }
        }

        // Push UI draw list last so it always draws over the swapchain.
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }

        r.end_frame()?;
        Ok(())
    }
}