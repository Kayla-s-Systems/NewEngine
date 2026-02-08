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

use crate::viewport_bridge::ViewportBridge;

use newengine_core::plugins::default_host_api;

use newengine_camera::frame;
use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind};
// Orbit controls are driven by UI via `ViewportBridge` to avoid leaking global input state.

#[derive(Debug, Clone, Copy, Default)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: [f32; 3],
}

impl OrbitCamera {
    #[inline]
    fn default_editor() -> Self {
        // Matches the old fixed camera roughly: eye=[2.6,1.8,2.6] looking at origin.
        Self {
            yaw: 0.7853982,  // 45 deg
            pitch: 0.55,
            dist: 4.1,
            target: [0.0, 0.0, 0.0],
        }
    }

    #[inline]
    fn apply_input(&mut self, dx_px: f32, dy_px: f32, wheel_y: f32, lmb_dragging: bool) {
        // Deltas are in *physical pixels*.
        const ROT_SENS: f32 = 0.0045;
        // Exponential zoom avoids "self-rotation" feeling and is stable across wheel deltas.
        const ZOOM_EXP_SENS: f32 = 0.0018;

        if lmb_dragging {
            if dx_px.is_finite() {
                self.yaw += dx_px * ROT_SENS;
            }
            if dy_px.is_finite() {
                self.pitch += dy_px * ROT_SENS;
            }
        }

        if wheel_y.is_finite() && wheel_y.abs() > 1e-6 {
            // Convention: wheel_y > 0 => zoom in.
            // exp(-x) => positive wheel shrinks distance.
            let factor = (-wheel_y * ZOOM_EXP_SENS).exp();
            if factor.is_finite() {
                self.dist *= factor;
            }
        }

        self.pitch = self.pitch.clamp(-1.54, 1.54);
        // Dist clamp keeps the camera stable; near/far are adjusted separately each frame.
        self.dist = self.dist.clamp(0.05, 250.0);
    }

    #[inline]
    fn eye(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();

        // Right-handed. yaw around +Y.
        [
            self.target[0] + self.dist * sy * cp,
            self.target[1] + self.dist * sp,
            self.target[2] + self.dist * cy * cp,
        ]
    }
}

// Input JSON structs removed: orbit input comes from UI.

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

#[derive(Clone, Copy)]
struct GridGpu {
    vb: newengine_core::render::BufferId,
    ubo: newengine_core::render::BufferId,

    bgl: newengine_core::render::BindGroupLayoutId,
    bg: newengine_core::render::BindGroupId,

    vs: newengine_core::render::ShaderId,
    fs: newengine_core::render::ShaderId,
    pipeline: newengine_core::render::PipelineId,

    vertex_count: u32,
    // Quantized scale used to decide when to rebuild VB.
    scale_q: u32,
}

pub struct EditorRenderController {
    model_center: [f32; 3],
    model_radius: f32,
    model_framed_once: bool,
    clear_color: [f32; 4],
    last_w: u32,
    last_h: u32,
    demo: Option<DemoGpu>,
    model: Option<ModelGpu>,
    grid: Option<GridGpu>,
    model_loaded_once: bool,

    orbit: OrbitCamera,

    assets: AssetServiceClient,

    viewport_bridge: std::sync::Arc<ViewportBridge>,
    viewport_rt: Option<newengine_core::render::RenderTargetId>,
    viewport_rt_extent: Extent2D,
}

impl EditorRenderController {
    #[inline]
    pub fn new(clear_color: [f32; 4], viewport_bridge: std::sync::Arc<ViewportBridge>) -> Self {
        Self {
            model_center: [0.0, 0.0, 0.0],
            model_radius: 1.0,
            model_framed_once: false,
            clear_color,
            last_w: 0,
            last_h: 0,
            demo: None,
            model: None,
            grid: None,
            model_loaded_once: false,

            orbit: OrbitCamera::default_editor(),

            assets: AssetServiceClient::new(default_host_api()),

            viewport_bridge,
            viewport_rt: None,
            viewport_rt_extent: Extent2D::new(0, 0),
        }
    }

    #[inline]
    fn quantize_grid_scale(s: f32) -> u32 {
        // Stable quantization: powers of 10 in millimeters..kilometers range.
        if !s.is_finite() || s <= 0.0 {
            return 0;
        }
        let exp = s.abs().log10().floor() as i32;
        (exp + 32).clamp(0, 64) as u32
    }

    #[inline]
    fn choose_grid_step(radius: f32) -> (f32, f32, f32) {
        // Returns (minor_step, major_step, half_extent).
        // The grid is built on XZ plane (Y=0) and adapts to the framed content.
        let r = radius.abs().max(1e-3);
        // Show ~20 major cells across the view.
        let desired_major = (r * 2.0) / 20.0;
        let exp = desired_major.log10().floor();
        let base = 10.0f32.powf(exp);

        // Snap major to {1,2,5} * 10^exp.
        let m = desired_major / base;
        let major = if m < 1.5 {
            1.0 * base
        } else if m < 3.5 {
            2.0 * base
        } else if m < 7.5 {
            5.0 * base
        } else {
            10.0 * base
        };

        let minor = major * 0.1;
        // Keep grid extent slightly larger than content.
        let half_extent = (r * 1.6).max(major * 10.0);
        (minor, major, half_extent)
    }

    fn build_grid(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        radius_hint: f32,
    ) -> EngineResult<()> {
        let (minor, major, half_extent) = Self::choose_grid_step(radius_hint);
        let scale_q = Self::quantize_grid_scale(major);

        if let Some(g) = self.grid {
            if g.scale_q == scale_q {
                return Ok(());
            }
            // Rebuild on scale changes.
            r.destroy_buffer(g.vb);
            r.destroy_buffer(g.ubo);
            r.destroy_bind_group(g.bg);
            r.destroy_bind_group_layout(g.bgl);
            r.destroy_shader(g.vs);
            r.destroy_shader(g.fs);
            r.destroy_pipeline(g.pipeline);
            self.grid = None;
        }

        // Vertex: pos.xyz + color.rgb
        let mut v: Vec<f32> = Vec::new();

        // Grid lines: XZ plane.
        // Minor lines are faint, major lines stronger, axes strongest.
        let axis_strength = 0.85f32;
        let major_strength = 0.28f32;
        let minor_strength = 0.12f32;

        let add_line = |out: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], s: f32| {
            out.extend_from_slice(&[a[0], a[1], a[2], s, s, s]);
            out.extend_from_slice(&[b[0], b[1], b[2], s, s, s]);
        };

        // Minor lines.
        let n_minor = (half_extent / minor).ceil() as i32;
        for i in -n_minor..=n_minor {
            let x = i as f32 * minor;
            let z = i as f32 * minor;

            // Skip where major will be drawn.
            let on_major_x = (x / major).round();
            if (x - on_major_x * major).abs() < 1e-5 {
                // handled by major pass
            } else {
                add_line(&mut v, [x, 0.0, -half_extent], [x, 0.0, half_extent], minor_strength);
            }

            let on_major_z = (z / major).round();
            if (z - on_major_z * major).abs() < 1e-5 {} else {
                add_line(&mut v, [-half_extent, 0.0, z], [half_extent, 0.0, z], minor_strength);
            }
        }

        // Major lines.
        let n_major = (half_extent / major).ceil() as i32;
        for i in -n_major..=n_major {
            let x = i as f32 * major;
            let z = i as f32 * major;
            add_line(&mut v, [x, 0.0, -half_extent], [x, 0.0, half_extent], major_strength);
            add_line(&mut v, [-half_extent, 0.0, z], [half_extent, 0.0, z], major_strength);
        }

        // Axes: X and Z.
        add_line(
            &mut v,
            [-half_extent, 0.0, 0.0],
            [half_extent, 0.0, 0.0],
            axis_strength,
        );
        add_line(
            &mut v,
            [0.0, 0.0, -half_extent],
            [0.0, 0.0, half_extent],
            axis_strength,
        );

        let vertex_count = (v.len() / 6) as u32;
        let mut vbytes: Vec<u8> = Vec::with_capacity(v.len() * 4);
        for f in v {
            vbytes.extend_from_slice(&f.to_ne_bytes());
        }

        let vb = r.create_buffer(
            BufferDesc::new(vbytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
                .with_label("editor_grid_vb"),
        )?;
        r.write_buffer(vb, 0, &vbytes)?;

        let ubo = r.create_buffer(
            BufferDesc::new(64, BufferUsage::Uniform, MemoryHint::CpuToGpu)
                .with_label("editor_grid_ubo"),
        )?;

        let bgl = r.create_bind_group_layout(
            BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer]).with_label("editor_grid_bgl"),
        )?;
        let bg = r.create_bind_group(
            BindGroupDesc::new(bgl)
                .with_label("editor_grid_bg")
                .with_uniform0(BufferBinding::new(ubo, 0, 64)),
        )?;

        let compiler = Compiler::new().ok_or_else(|| EngineError::other("shaderc: Compiler"))?;

        const VS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_col;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
} u;

layout(location = 0) out vec3 v_col;

void main() {
    v_col = a_col;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
"#;

        const FS_SRC: &str = r#"#version 450
layout(location = 0) in vec3 v_col;
layout(location = 0) out vec4 o_col;

void main() {
    // Subtle grid (pre-multiplied style isn't available here), keep alpha = 1.
    o_col = vec4(v_col, 1.0);
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

        let stride = (6 * std::mem::size_of::<f32>()) as u32;
        let layout = VertexLayout::new(
            stride,
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
                .with_label("editor_grid_pipeline")
                .with_topology(PrimitiveTopology::LineList)
                .with_vertex_layouts(vec![layout])
                .with_bind_group_layouts(vec![bgl]),
        )?;

        self.grid = Some(GridGpu {
            vb,
            ubo,
            bgl,
            bg,
            vs,
            fs,
            pipeline,
            vertex_count,
            scale_q,
        });

        Ok(())
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

        self.model_center = center;
        self.model_radius = radius;
        // Keep orbit target synced even before first frame-all.
        self.orbit.target = center;

        let stride = 6 * std::mem::size_of::<f32>();
        let mut vbytes: Vec<u8> = Vec::with_capacity(pos.len() * stride);

        for (p, n) in pos.iter().zip(nrm.iter()) {
            // Keep authoring scale. Camera + near/far use bounds.
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

        // Camera matrices are written per-frame; keep ubo zeroed for now.
        r.write_buffer(ubo, 0, &[0u8; 64])?;

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

        self.model_center = center;
        self.model_radius = radius;
        // Reset framing on new content.
        self.model_framed_once = false;

        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for EditorRenderController {
    fn id(&self) -> &'static str {
        "app.render_controller"
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();

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

            let (dx_px, dy_px, wheel_y, _hovered, dragging) = self.viewport_bridge.read_orbit_input();
            self.orbit.apply_input(dx_px, dy_px, wheel_y, dragging);

            if let Some(rt) = self.viewport_rt {
                r.begin_render_target(BeginRenderTargetDesc::new(rt))?;

                let extent = Extent2D::new(vp_w, vp_h);
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(0, 0, vp_w as i32, vp_h as i32))?;

                // Build the camera frame (universal math: works the same in editor and game).
                let aspect = vp_w as f32 / (vp_h.max(1) as f32);
                let fov_y = 60.0f32.to_radians();

                let has_model = self.model.is_some();
                let target = if has_model { self.model_center } else { [0.0, 0.0, 0.0] };
                let radius = if has_model { self.model_radius.max(1e-6) } else { 1.0 };

                // Keep orbit target synced.
                self.orbit.target = target;

                // One-time frame-all when content appears.
                if has_model && !self.model_framed_once {
                    let dist = frame::fit_distance_for_sphere_perspective(fov_y, aspect, radius, 1.15);
                    self.orbit.dist = dist;
                    self.model_framed_once = true;
                }

                // Clamp: minimum depends on radius; maximum is a safety cap.
                self.orbit.dist = self.orbit.dist.clamp(radius * 1.05, 250_000.0);

                let (near, far) = frame::auto_near_far(self.orbit.dist, radius);
                let proj = Self::mat4_perspective(fov_y, aspect, near, far);
                let eye = self.orbit.eye();
                let view = Self::mat4_look_at(eye, target, [0.0, 1.0, 0.0]);
                let mvp = Self::mat4_mul(proj, view);

                // Grid: build + draw first so the model draws on top (no depth yet).
                self.build_grid(&mut **r, radius)?;
                if let Some(grid) = self.grid {
                    let mut ubytes: Vec<u8> = Vec::with_capacity(64);
                    for f in mvp {
                        ubytes.extend_from_slice(&f.to_ne_bytes());
                    }
                    r.write_buffer(grid.ubo, 0, &ubytes)?;
                    r.set_pipeline(grid.pipeline)?;
                    r.set_bind_group(0, grid.bg)?;
                    r.set_vertex_buffer(0, BufferSlice::new(grid.vb, 0))?;
                    r.draw(newengine_core::render::DrawArgs::new(grid.vertex_count))?;
                }

                if let Some(model) = self.model {
                    let mut ubytes: Vec<u8> = Vec::with_capacity(64);
                    for f in mvp {
                        ubytes.extend_from_slice(&f.to_ne_bytes());
                    }
                    r.write_buffer(model.ubo, 0, &ubytes)?;

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