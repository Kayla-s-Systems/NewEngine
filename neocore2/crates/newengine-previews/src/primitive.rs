#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};
use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::error::{EngineError, EngineResult};
use newengine_core::render::{
    BeginRenderTargetDesc, BindGroupDesc, BindGroupLayoutDesc, BindingKind, BufferBinding,
    BufferDesc, BufferUsage, DrawIndexedArgs, Extent2D, IndexFormat, PipelineDesc,
    PrimitiveTopology, RectI32, RenderApi, RenderTargetDesc, RenderTargetId, ShaderDesc,
    ShaderStage, TextureFormat, UiTexId, VertexAttribute, VertexFormat, VertexLayout,
};
use newengine_math::collections::FxHashMap;
use newengine_math::{Mat4, Vec3};
use newengine_plugin_host::default_host_api;
use newengine_primitives::{PrimitiveId, PrimitiveRegistry, PrimitiveVertex};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitivePreviewSize {
    S32,
    S48,
    S64,
    S96,
    S128,
}

impl PrimitivePreviewSize {
    #[inline]
    pub const fn px(self) -> u32 {
        match self {
            Self::S32 => 32,
            Self::S48 => 48,
            Self::S64 => 64,
            Self::S96 => 96,
            Self::S128 => 128,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct PreviewUbo {
    mvp: [[f32; 4]; 4],
    color: [f32; 4],
    light_dir: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
struct GpuMesh {
    vb: newengine_core::render::BufferId,
    ib: newengine_core::render::BufferId,
    index_count: u32,
}

#[derive(Debug)]
struct CpuMesh {
    vertices: Vec<PrimitiveVertex>,
    indices: Vec<u32>,
}

#[derive(Debug)]
struct Slot {
    size: PrimitivePreviewSize,
    // Allocated lazily on the render thread.
    rt: Option<RenderTargetId>,
    ui_tex: UiTexId,
    mesh: Option<GpuMesh>,
    cpu: Option<CpuMesh>,

    radius: f32,
    seed: u32,
    dirty: bool,
}

/// Engine-side dynamic primitive preview renderer.
///
/// Apps should:
/// - call `request(id, size)` in UI code, store/use returned `UiTexId`.
/// - call `pump(render_api, frame_dt)` once per frame (inside the active frame).
#[derive(Debug)]
pub struct PrimitivePreviewService {
    reg: PrimitiveRegistry,
    slots: FxHashMap<(PrimitiveId, PrimitivePreviewSize), Slot>,

    // GPU state
    bgl: Option<newengine_core::render::BindGroupLayoutId>,
    bg: Option<newengine_core::render::BindGroupId>,
    ubo: Option<newengine_core::render::BufferId>,
    pipeline: Option<newengine_core::render::PipelineId>,

    // animation
    t: f32,
}

impl Default for PrimitivePreviewService {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimitivePreviewService {
    #[inline]
    pub fn new() -> Self {
        Self {
            reg: PrimitiveRegistry::with_builtins(),
            slots: Default::default(),
            bgl: None,
            bg: None,
            ubo: None,
            pipeline: None,
            t: 0.0,
        }
    }

    /// Request a preview texture for a primitive.
    ///
    /// The returned `UiTexId` is stable for the lifetime of the slot.
    /// If the preview is not yet rendered, this may temporarily return `UiTexId(0)`.
    pub fn request(&mut self, id: PrimitiveId, size: PrimitivePreviewSize) -> UiTexId {
        let key = (id, size);
        if let Some(slot) = self.slots.get(&key) {
            return slot.ui_tex;
        }

        let _seed = estimate_radius_and_seed(id);

        // Build CPU mesh now (deterministic), upload later (render thread).
        let (radius, seed) = estimate_radius_and_seed(id);
        let mesh = match self.reg.build_mesh(id) {
            Ok(m) => m,
            Err(_e) => {
                // If we cannot build a mesh for this id, we can't provide a preview.
                // Returning 0 is a safe "no preview" fallback for consumers.
                return UiTexId(0);
            }
        };

        self.slots.insert(
            key,
            Slot {
                size,
                rt: None,
                ui_tex: UiTexId(0),
                mesh: None,
                cpu: Some(CpuMesh {
                    vertices: mesh.vertices,
                    indices: mesh.indices,
                }),
                radius,
                seed,
                dirty: true,
            },
        );

        UiTexId(0)
    }

    /// Render dirty previews. Call once per frame (inside an active frame).
    pub fn pump(&mut self, r: &mut dyn RenderApi, dt: f32) -> EngineResult<()> {
        // Do not bake/compile preview shaders until somebody actually requested a
        // preview. This keeps the first editor frame independent from optional
        // thumbnail rendering and prevents local `glslc` failures from aborting
        // startup before the viewport/UI is visible.
        if self.slots.is_empty() {
            return Ok(());
        }

        self.ensure_gpu_state(r)?;
        self.t = (self.t + dt).min(10_000.0);

        let pipeline = self
            .pipeline
            .ok_or_else(|| EngineError::other("primitive preview: missing pipeline"))?;
        let bg = self
            .bg
            .ok_or_else(|| EngineError::other("primitive preview: missing bind group"))?;
        let ubo = self
            .ubo
            .ok_or_else(|| EngineError::other("primitive preview: missing ubo"))?;

        // We render a small budget each frame; keep it simple for now.
        let keys: Vec<(PrimitiveId, PrimitivePreviewSize)> = self.slots.keys().copied().collect();
        for k in keys {
            let Some(slot) = self.slots.get_mut(&k) else {
                continue;
            };

            // Lazily allocate GPU resources.
            if slot.rt.is_none() || slot.mesh.is_none() {
                let cpu = slot
                    .cpu
                    .take()
                    .ok_or_else(|| EngineError::other("primitive preview: missing cpu mesh"))?;

                let gpu = upload_mesh(r, &cpu.vertices, &cpu.indices)?;

                let extent = Extent2D::new(slot.size.px(), slot.size.px());
                let rt = r.create_render_target(
                    RenderTargetDesc::new(extent, TextureFormat::Rgba8Unorm)
                        .with_depth(TextureFormat::Depth24Stencil8)
                        .with_label("primitive_preview_rt"),
                )?;
                let ui_tex = r.render_target_ui_tex_id(rt)?;

                slot.mesh = Some(gpu);
                slot.rt = Some(rt);
                slot.ui_tex = ui_tex;
                slot.dirty = true;
            }

            // Dynamic: subtle turntable motion.
            // If you want fully static previews, gate this behind a setting.
            if !slot.dirty {
                slot.dirty = true;
            }

            let angle = self.t * 0.65 + (slot.seed as f32) * 0.01;
            let mvp = compute_mvp(slot.radius.max(0.001), angle);

            let u = PreviewUbo {
                mvp: mvp.to_cols_array_2d(),
                color: [0.86, 0.87, 0.92, 1.0],
                light_dir: [0.35, 0.85, 0.45, 0.0],
            };
            r.write_buffer(ubo, 0, bytemuck::bytes_of(&u))?;

            let clear = [0.10, 0.105, 0.11, 1.0];
            let rt = slot
                .rt
                .ok_or_else(|| EngineError::other("primitive preview: missing render target"))?;
            let mesh = slot
                .mesh
                .ok_or_else(|| EngineError::other("primitive preview: missing gpu mesh"))?;

            r.begin_render_target(
                BeginRenderTargetDesc::new(rt)
                    .with_clear_color(clear)
                    .with_clear_depth(1.0),
            )?;

            let extent = Extent2D::new(slot.size.px(), slot.size.px());
            r.set_viewport(newengine_core::render::Viewport::full(extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                extent.width as i32,
                extent.height as i32,
            ))?;

            r.set_pipeline(pipeline)?;
            r.set_bind_group(0, bg)?;

            r.set_vertex_buffer(0, newengine_core::render::BufferSlice::new(mesh.vb, 0))?;
            r.set_index_buffer(
                newengine_core::render::BufferSlice::new(mesh.ib, 0),
                IndexFormat::U32,
            )?;

            r.draw_indexed(DrawIndexedArgs::new(mesh.index_count))?;

            r.end_render_target()?;

            slot.dirty = false;
        }

        Ok(())
    }

    fn ensure_gpu_state(&mut self, r: &mut dyn RenderApi) -> EngineResult<()> {
        if self.pipeline.is_some() {
            return Ok(());
        }

        let vs_words = Self::load_or_compile_spv_words(
            "shaders/preview/primitive_preview.vert",
            ShaderStage::Vertex,
        )?;
        let fs_words = Self::load_or_compile_spv_words(
            "shaders/preview/primitive_preview.frag",
            ShaderStage::Fragment,
        )?;

        // UBO/resources are allocated only after shader baking succeeds.
        // This keeps the GPU state clean when a local shader compiler crashes/fails.
        let ubo = r.create_buffer(
            BufferDesc::new(
                std::mem::size_of::<PreviewUbo>() as u64,
                BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
                .with_label("primitive_preview_ubo"),
        )?;

        let bgl = r.create_bind_group_layout(
            BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer])
                .with_label("primitive_preview_bgl"),
        )?;

        let bg = r.create_bind_group(
            BindGroupDesc::new(bgl)
                .with_label("primitive_preview_bg")
                .with_uniform0(BufferBinding::new(
                    ubo,
                    0,
                    std::mem::size_of::<PreviewUbo>() as u64,
                )),
        )?;

        let vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", vs_words)
                .with_label("primitive_preview_vs"),
        )?;
        let fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", fs_words)
                .with_label("primitive_preview_fs"),
        )?;

        let stride = std::mem::size_of::<PrimitiveVertex>() as u32;
        let attrs = vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(1, 12, VertexFormat::Float32x3),
        ];

        let pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, TextureFormat::Rgba8Unorm)
                .with_label("primitive_preview_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![VertexLayout::new(stride, attrs)])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth24Stencil8),
        )?;

        self.ubo = Some(ubo);
        self.bgl = Some(bgl);
        self.bg = Some(bg);
        self.pipeline = Some(pipeline);

        Ok(())
    }

    /// Converts raw SPIR-V bytes into aligned u32 words.
    ///
    /// SPIR-V binary format is defined as a sequence of 32-bit words.
    /// The input length must be divisible by 4.
    fn spirv_bytes_to_words(bytes: &[u8]) -> EngineResult<Vec<u32>> {
        if bytes.is_empty() {
            return Err(EngineError::other(
                "primitive preview: SPIR-V bytecode is empty",
            ));
        }
        if bytes.len() % 4 != 0 {
            return Err(EngineError::other(
                "primitive preview: SPIR-V bytecode length must be divisible by 4",
            ));
        }

        let mut out = Vec::with_capacity(bytes.len() / 4);
        for c in bytes.chunks_exact(4) {
            out.push(u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        }
        Ok(out)
    }

    fn load_or_compile_spv_words(logical_path: &str, stage: ShaderStage) -> EngineResult<Vec<u32>> {
        let assets = AssetServiceClient::new(default_host_api());

        let id = assets.import_v1(logical_path).map_err(|e| {
            EngineError::other(format!("asset.import_v1 failed path='{logical_path}' err='{e}'"))
        })?;

        wait_ready(&assets, &id, Duration::from_millis(500)).map_err(|e| {
            EngineError::other(format!(
                "asset.wait_ready failed path='{logical_path}' err='{e:?}'"
            ))
        })?;

        let (_meta, payload) = assets.blob_wire_v1(&id).map_err(|e| {
            EngineError::other(format!(
                "asset.blob_wire_v1 failed path='{logical_path}' err='{e}'"
            ))
        })?;

        let src = std::str::from_utf8(&payload).map_err(|_| {
            EngineError::other(format!("shader source is not utf8 path='{logical_path}'"))
        })?;

        let cache_dir = shader_cache_dir();
        std::fs::create_dir_all(&cache_dir).map_err(|e| {
            EngineError::other(format!(
                "shader cache: create_dir_all failed dir='{}' err='{e}'",
                cache_dir.display()
            ))
        })?;

        let key = shader_cache_key(src, stage, "main");
        let out_path = cache_dir.join(shader_cache_filename(logical_path, stage, &key));

        if let Ok(bytes) = std::fs::read(&out_path) {
            if let Ok(words) = Self::spirv_bytes_to_words(&bytes) {
                return Ok(words);
            }
        }

        let words = newengine_shader_compiler::compile_glsl_to_spirv(
            stage,
            logical_path,
            "main",
            src,
        )
        .map_err(|e| {
            EngineError::other(format!(
                "shader compile failed path='{logical_path}' err='{e}'"
            ))
        })?;

        let bytes = spirv_words_to_bytes(&words);
        let _ = atomic_write(&out_path, &bytes);

        Ok(words)
    }
}

fn shader_cache_dir() -> PathBuf {
    if let Some(v) = std::env::var_os("NEWENGINE_SHADER_CACHE_DIR") {
        return PathBuf::from(v);
    }
    std::env::var_os("NEWENGINE_CACHE_FILES")
        .or_else(|| std::env::var_os("CACHE_FILES"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cache"))
        .join("shaders")
        .join("previews")
}

fn spirv_words_to_bytes(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

fn suffix(stage: ShaderStage) -> &'static str {
    match stage {
        ShaderStage::Vertex => "vert",
        ShaderStage::Fragment => "frag",
        ShaderStage::Compute => "comp",
    }
}

fn shader_cache_key(src: &str, stage: ShaderStage, entry: &str) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(format!("{stage:?}").as_bytes());
    h.update(b"\0");
    h.update(entry.as_bytes());
    h.update(b"\0");
    h.update(src.as_bytes());
    *h.finalize().as_bytes()
}

fn shader_cache_filename(logical_path: &str, stage: ShaderStage, key: &[u8; 32]) -> String {
    let stem = Path::new(logical_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("shader");
    format!("{stem}.{}.{}.spv", suffix(stage), hex16(key))
}

fn hex16(key: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];
    for (i, b) in key[..16].iter().copied().enumerate() {
        out[i * 2] = HEX[(b >> 4) as usize];
        out[i * 2 + 1] = HEX[(b & 0x0F) as usize];
    }
    String::from_utf8_lossy(&out).to_string()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("spv.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
    }
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn upload_mesh(
    r: &mut dyn RenderApi,
    vertices: &[PrimitiveVertex],
    indices: &[u32],
) -> EngineResult<GpuMesh> {
    if vertices.is_empty() || indices.is_empty() {
        return Err(EngineError::other("primitive preview: empty mesh"));
    }

    let vb = r.create_buffer(
        BufferDesc::new(
            (vertices.len() * std::mem::size_of::<PrimitiveVertex>()) as u64,
            BufferUsage::Vertex,
            newengine_core::render::MemoryHint::CpuToGpu,
        )
            .with_label("primitive_preview_vb"),
    )?;
    let ib = r.create_buffer(
        BufferDesc::new(
            (indices.len() * std::mem::size_of::<u32>()) as u64,
            BufferUsage::Index,
            newengine_core::render::MemoryHint::CpuToGpu,
        )
            .with_label("primitive_preview_ib"),
    )?;

    r.write_buffer(vb, 0, bytemuck::cast_slice(vertices))?;
    r.write_buffer(ib, 0, bytemuck::cast_slice(indices))?;

    Ok(GpuMesh {
        vb,
        ib,
        index_count: indices.len() as u32,
    })
}

fn compute_mvp(radius: f32, angle: f32) -> Mat4 {
    // Camera: turntable around origin.
    let dist = (radius * 3.2).clamp(0.75, 50.0);
    let eye = Vec3::new(angle.cos() * dist, dist * 0.72, angle.sin() * dist);
    let center = Vec3::ZERO;
    let up = Vec3::Y;

    let view = Mat4::look_at_rh(eye, center, up);
    let proj = Mat4::perspective_rh(40.0_f32.to_radians(), 1.0, 0.01, 500.0);

    // Backend is Vulkan; RH projection is OK for the current pipeline conventions.
    proj * view
}

fn estimate_radius_and_seed(id: PrimitiveId) -> (f32, u32) {
    // Deterministic heuristics. A future version can compute bounds from mesh.
    let seed = (id.0 as u32) ^ ((id.0 >> 32) as u32);
    (1.0, seed)
}
