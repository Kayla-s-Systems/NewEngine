#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    require_render_api, BeginFrameDesc, BindGroupDesc, BindGroupLayoutDesc, BindingKind,
    BufferBinding, BufferDesc, BufferSlice, BufferUsage, DrawIndexedArgs, Extent2D, IndexFormat,
    MemoryHint, PipelineDesc, PrimitiveTopology, RectI32, ShaderDesc, ShaderStage, TextureFormat,
    VertexAttribute, VertexFormat, VertexLayout, Viewport,
};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

use newengine_assets::{AssetState, Model3dFormat, Model3dReader};

use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind};

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
    clear_color: [f32; 4],
    last_w: u32,
    last_h: u32,
    demo: Option<DemoGpu>,
    model: Option<ModelGpu>,
    model_loaded_once: bool,
}

impl EditorRenderController {
    #[inline]
    pub fn new(clear_color: [f32; 4]) -> Self {
        Self {
            clear_color,
            last_w: 0,
            last_h: 0,
            demo: None,
            model: None,
            model_loaded_once: false,
        }
    }

    fn load_model_blob(
        ctx: &ModuleCtx<'_, impl Send + 'static>,
        logical_path: &str,
        timeout_ms: u64,
    ) -> EngineResult<Option<std::sync::Arc<newengine_assets::AssetBlob>>> {
        let Some(am) = ctx.resources().get::<newengine_core::assets::AssetManager>() else {
            return Ok(None);
        };

        let store = am.store();
        let id = match store.load_path(logical_path) {
            Ok(id) => id,
            Err(e) => {
                log::warn!("model: asset.load failed path='{logical_path}' err='{e}'");
                return Ok(None);
            }
        };

        let t0 = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            am.pump();
            match store.state(id) {
                AssetState::Ready => return Ok(store.get_blob(id)),
                AssetState::Failed(e) => {
                    return Err(EngineError::other(format!(
                        "model: import failed path='{logical_path}' err='{e}'"
                    )));
                }
                _ => {
                    if t0.elapsed() >= timeout {
                        log::warn!("model: load timeout path='{logical_path}'");
                        return Ok(None);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
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
    fn mat4_perspective(fov_y_rad: f32, aspect: f32, z_near: f32, z_far: f32) -> [f32; 16] {
        let f = 1.0 / (0.5 * fov_y_rad).tan();
        let nf = 1.0 / (z_near - z_far);

        // Column-major, Vulkan clip space Z: [0..1]
        // Row-major form:
        // [ f/aspect, 0,   0,              0 ]
        // [ 0,       -f,  0,              0 ]
        // [ 0,        0,  z_far*nf,       -1 ]
        // [ 0,        0,  z_far*z_near*nf, 0 ]
        [
            f / aspect, 0.0, 0.0, 0.0, //
            0.0, -f, 0.0, 0.0,        //
            0.0, 0.0, z_far * nf, z_far * z_near * nf, //
            0.0, 0.0, -1.0, 0.0,      //
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
        ctx: &ModuleCtx<'_, impl Send + 'static>,
        r: &mut dyn newengine_core::render::RenderApi,
        target: Extent2D,
    ) -> EngineResult<()> {
        if self.model.is_some() || self.model_loaded_once {
            return Ok(());
        }

        self.model_loaded_once = true;

        const MODEL_PATH: &str = "models/demo.obj";

        let Some(blob) = Self::load_model_blob(ctx, MODEL_PATH, 750)? else {
            log::warn!("model: missing '{MODEL_PATH}'. Add an .obj under assets/models/demo.obj to see 3D.");
            return Ok(());
        };

        let model = Model3dReader::from_blob_parts(blob.meta_json.as_ref(), &blob.payload)
            .map_err(|e| EngineError::other(format!("model: decode failed: {e}")))?;

        if model.format != Model3dFormat::Ne3d {
            log::warn!(
                "model: '{MODEL_PATH}' imported as {:?} (need NE3D for rendering)",
                model.format
            );
            return Ok(());
        }

        let (pos, nrm, idx) = Self::decode_ne3d_mesh(&model.payload)?;
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
        let ext = [
            (bb_max[0] - bb_min[0]).abs(),
            (bb_max[1] - bb_min[1]).abs(),
            (bb_max[2] - bb_min[2]).abs(),
        ];
        let radius = (0.5 * ext[0].max(ext[1]).max(ext[2])).max(0.001);
        let inv_radius = 1.0 / radius;

        let stride = 6 * std::mem::size_of::<f32>();
        let mut vbytes: Vec<u8> = Vec::with_capacity(pos.len() * stride);

        for (p, n) in pos.iter().zip(nrm.iter()) {
            let px = (p[0] - center[0]) * inv_radius;
            let py = (p[1] - center[1]) * inv_radius;
            let pz = (p[2] - center[2]) * inv_radius;

            vbytes.extend_from_slice(&px.to_ne_bytes());
            vbytes.extend_from_slice(&py.to_ne_bytes());
            vbytes.extend_from_slice(&pz.to_ne_bytes());
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
                .with_depth(TextureFormat::Depth32Float)
                .with_label("editor_model_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout])
                .with_bind_group_layouts(vec![bgl]),
        )?;

        let aspect = if target.height == 0 {
            1.0
        } else {
            target.width as f32 / target.height as f32
        };

        let proj = Self::mat4_perspective(60.0f32.to_radians(), aspect, 0.01, 1000.0);
        let view = Self::mat4_look_at([2.6, 1.8, 2.6], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let mvp = Self::mat4_mul(proj, view);

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

        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }

        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            r.resize(w, h)?;
        }

        self.build_demo(&mut **r)?;
        if w > 0 && h > 0 {
            self.build_model(ctx, &mut **r, Extent2D::new(w, h))?;
        }

        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;

        if w > 0 && h > 0 {
            let extent = Extent2D::new(w, h);
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(0, 0, w as i32, h as i32))?;

            if let Some(model) = self.model {
                let aspect = w as f32 / (h.max(1) as f32);
                let proj = Self::mat4_perspective(60.0f32.to_radians(), aspect, 0.01, 1000.0);

                let a = (ctx.frame.unwrap().frame_index as f32) * 0.01;
                let rot = Self::mat4_rotation_y(a);
                let view = Self::mat4_look_at([2.6, 1.8, 2.6], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);

                let mvp = Self::mat4_mul(Self::mat4_mul(proj, view), rot);

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
        }

        r.end_frame()?;
        Ok(())
    }
}