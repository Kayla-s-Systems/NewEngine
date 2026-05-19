#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady/FPS runtime-lit material-domain provider.
//!
//! This crate is a feature package, not a renderer backend. It owns the current
//! GameReady shader asset paths and pipeline presets, then creates backend-neutral
//! `RenderApi` resources for the reusable runtime render controller.

use newengine_assets::AssetServiceClient;
use newengine_render_api::*;
use newengine_material_domain_api::{
    LitPipeline, MaterialDomainError, MaterialDomainResult, MaterialGpuPipeline,
    MaterialGpuPipelineKey, MaterialGpuPipelineProvider, MaterialPipelineBuildProfile,
    MaterialRenderDevice, LIT_INSTANCE_VERTEX_STRIDE,
};
use newengine_plugin_host::default_host_api;
use newengine_primitives::PrimitiveVertex;

pub const GAME_READY_LIT_PIPELINE_KEY: MaterialGpuPipelineKey =
    MaterialGpuPipelineKey::new("newengine.material_domain.gameready.runtime_lit");

#[derive(Default)]
pub struct GameReadyLitMaterialDomainProvider {
    bytecode: Option<GameReadyLitShaderBytecodeSet>,
    pipeline: Option<LitPipeline>,
}

impl GameReadyLitMaterialDomainProvider {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn require_bytecode(&mut self) -> MaterialDomainResult<GameReadyLitShaderBytecodeSet> {
        if let Some(shader_set) = self.bytecode.clone() {
            return Ok(shader_set);
        }

        let shader_set = GameReadyLitShaderBytecodeSet::load_and_compile()?;
        self.bytecode = Some(shader_set.clone());
        Ok(shader_set)
    }

    fn build_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<LitPipeline> {
        let shader_set = self.require_bytecode()?;

        // Allocate GPU resources only after shader baking succeeds. Runtime shader
        // compilation is still optional during startup; a local glslc crash must not
        // leave half-created backend objects before the controller fails soft.
        let bgl = r.create_bind_group_layout(
            BindGroupLayoutDesc::new(vec![
                BindingKind::UniformBuffer,
                BindingKind::Texture2D,
                BindingKind::Texture2D,
                BindingKind::Texture2D,
                BindingKind::Texture2D,
                BindingKind::Sampler,
            ])
            .with_label("gameready_lit_bgl"),
        )?;
        let white_texture = r.create_texture(
            TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
                .with_label("gameready_white_tex")
                .with_data(vec![255, 255, 255, 255]),
        )?;
        let flat_normal_texture = r.create_texture(
            TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
                .with_label("gameready_flat_normal_tex")
                .with_data(vec![128, 128, 255, 255]),
        )?;
        let repeat_sampler = r.create_sampler(
            SamplerDesc::default()
                .with_label("gameready_repeat_sampler")
                .with_repeat(),
        )?;
        let clamp_sampler = r.create_sampler(
            SamplerDesc::default()
                .with_label("gameready_clamp_sampler")
                .with_address_u(AddressMode::ClampToEdge)
                .with_address_v(AddressMode::ClampToEdge)
                .with_address_w(AddressMode::ClampToEdge),
        )?;
        let vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", shader_set.vs).with_label("gameready_lit_vs"),
        )?;
        let fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", shader_set.fs).with_label("gameready_lit_fs"),
        )?;
        let terrain_fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", shader_set.terrain_fs)
                .with_label("gameready_terrain_surface_fs"),
        )?;
        let shadow_vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", shader_set.shadow_vs)
                .with_label("gameready_sun_shadow_depth_vs"),
        )?;
        let shadow_fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", shader_set.shadow_fs)
                .with_label("gameready_sun_shadow_depth_fs"),
        )?;
        let instanced_vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", shader_set.instanced_vs)
                .with_label("gameready_lit_instanced_vs"),
        )?;
        let instanced_fs = r.create_shader(
            ShaderDesc::new(ShaderStage::Fragment, "main", shader_set.instanced_fs)
                .with_label("gameready_lit_instanced_fs"),
        )?;
        let shadow_instanced_vs = r.create_shader(
            ShaderDesc::new(ShaderStage::Vertex, "main", shader_set.shadow_instanced_vs)
                .with_label("gameready_sun_shadow_instanced_vs"),
        )?;

        let stride = std::mem::size_of::<PrimitiveVertex>() as u32;
        let layout = VertexLayout::new(
            stride,
            vec![
                VertexAttribute::new(0, 0, VertexFormat::Float32x3),
                VertexAttribute::new(1, 12, VertexFormat::Float32x3),
                VertexAttribute::new(2, 24, VertexFormat::Float32x2),
            ],
        );

        let instance_layout = VertexLayout::new(
            LIT_INSTANCE_VERTEX_STRIDE,
            vec![
                VertexAttribute::new(5, 0, VertexFormat::Float32x4),
                VertexAttribute::new(6, 16, VertexFormat::Float32x4),
                VertexAttribute::new(7, 32, VertexFormat::Float32x4),
                VertexAttribute::new(8, 48, VertexFormat::Float32x4),
                VertexAttribute::new(9, 64, VertexFormat::Float32x4),
                VertexAttribute::new(10, 80, VertexFormat::Float32x4),
                VertexAttribute::new(11, 96, VertexFormat::Float32x4),
                VertexAttribute::new(12, 112, VertexFormat::Float32x4),
                VertexAttribute::new(13, 128, VertexFormat::Float32x4),
                VertexAttribute::new(14, 144, VertexFormat::Float32x4),
                VertexAttribute::new(15, 160, VertexFormat::Float32x4),
                VertexAttribute::new(16, 176, VertexFormat::Float32x4),
            ],
        )
        .per_instance();

        let pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, profile.scene_hdr_color_format)
                .with_label("gameready_lit_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout.clone()])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float),
        )?;

        let double_sided_pipeline = r.create_pipeline(
            PipelineDesc::new(vs, fs, profile.scene_hdr_color_format)
                .with_label("gameready_lit_pipeline_double_sided")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout.clone()])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float)
                .with_cull_mode(RasterCullMode::None),
        )?;

        let terrain_pipeline = r.create_pipeline(
            PipelineDesc::new(vs, terrain_fs, profile.scene_hdr_color_format)
                .with_label("gameready_terrain_surface_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout.clone()])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float),
        )?;

        let shadow_pipeline = r.create_pipeline(
            PipelineDesc::new(shadow_vs, shadow_fs, profile.shadow_map_color_format)
                .with_label("gameready_sun_shadow_depth_pipeline")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout.clone()])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float),
        )?;

        let shadow_double_sided_pipeline = r.create_pipeline(
            PipelineDesc::new(shadow_vs, shadow_fs, profile.shadow_map_color_format)
                .with_label("gameready_sun_shadow_depth_pipeline_double_sided")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(vec![layout.clone()])
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float)
                .with_cull_mode(RasterCullMode::None),
        )?;

        let instanced_layouts = vec![layout.clone(), instance_layout.clone()];
        let instanced_pipeline = r.create_pipeline(
            PipelineDesc::new(instanced_vs, instanced_fs, profile.scene_hdr_color_format)
                .with_label("gameready_lit_pipeline_instanced")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(instanced_layouts.clone())
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float),
        )?;

        let instanced_double_sided_pipeline = r.create_pipeline(
            PipelineDesc::new(instanced_vs, instanced_fs, profile.scene_hdr_color_format)
                .with_label("gameready_lit_pipeline_instanced_double_sided")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(instanced_layouts.clone())
                .with_bind_group_layouts(vec![bgl])
                .with_depth(TextureFormat::Depth32Float)
                .with_cull_mode(RasterCullMode::None),
        )?;

        let shadow_instanced_pipeline = r.create_pipeline(
            PipelineDesc::new(
                shadow_instanced_vs,
                shadow_fs,
                profile.shadow_map_color_format,
            )
            .with_label("gameready_sun_shadow_depth_pipeline_instanced")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(instanced_layouts.clone())
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
        )?;

        let shadow_instanced_double_sided_pipeline = r.create_pipeline(
            PipelineDesc::new(
                shadow_instanced_vs,
                shadow_fs,
                profile.shadow_map_color_format,
            )
            .with_label("gameready_sun_shadow_depth_pipeline_instanced_double_sided")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(instanced_layouts)
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float)
            .with_cull_mode(RasterCullMode::None),
        )?;

        Ok(LitPipeline {
            bgl,
            white_texture,
            flat_normal_texture,
            repeat_sampler,
            clamp_sampler,
            vs,
            fs,
            terrain_fs,
            shadow_vs,
            shadow_fs,
            pipeline,
            double_sided_pipeline,
            terrain_pipeline,
            shadow_pipeline,
            shadow_double_sided_pipeline,
            instanced_vs,
            instanced_fs,
            shadow_instanced_vs,
            instanced_pipeline,
            instanced_double_sided_pipeline,
            shadow_instanced_pipeline,
            shadow_instanced_double_sided_pipeline,
        })
    }
}

impl MaterialGpuPipelineProvider for GameReadyLitMaterialDomainProvider {
    #[inline]
    fn key(&self) -> MaterialGpuPipelineKey {
        GAME_READY_LIT_PIPELINE_KEY
    }

    fn require_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<MaterialGpuPipeline> {
        if let Some(pipeline) = self.pipeline {
            return Ok(MaterialGpuPipeline::Lit(pipeline));
        }

        let pipeline = self.build_pipeline(profile, r)?;
        self.pipeline = Some(pipeline);
        Ok(MaterialGpuPipeline::Lit(pipeline))
    }
}

#[derive(Clone)]
struct GameReadyLitShaderBytecodeSet {
    vs: Vec<u32>,
    fs: Vec<u32>,
    terrain_fs: Vec<u32>,
    shadow_vs: Vec<u32>,
    shadow_fs: Vec<u32>,
    instanced_vs: Vec<u32>,
    instanced_fs: Vec<u32>,
    shadow_instanced_vs: Vec<u32>,
}

impl GameReadyLitShaderBytecodeSet {
    fn load_and_compile() -> MaterialDomainResult<Self> {
        let vs_src = load_text_asset("shaders/game_lit_shadowed_v1.vert")?;
        let fs_src = load_text_asset("shaders/game_lit_shadowed_v1.frag")?;
        let terrain_fs_src = load_text_asset("shaders/game_terrain_surface_v1.frag")?;
        let shadow_vs_src = load_text_asset("shaders/game_sun_shadow_depth_v1.vert")?;
        let shadow_fs_src = load_text_asset("shaders/game_sun_shadow_depth_v1.frag")?;
        let instanced_vs_src = load_text_asset("shaders/game_lit_instanced_v1.vert")?;
        let instanced_fs_src = load_text_asset("shaders/game_lit_instanced_v1.frag")?;
        let shadow_instanced_vs_src =
            load_text_asset("shaders/game_sun_shadow_depth_instanced_v1.vert")?;

        Ok(Self {
            vs: compile_glsl(ShaderStage::Vertex, "game_lit_shadowed_v1.vert", &vs_src)?,
            fs: compile_glsl(ShaderStage::Fragment, "game_lit_shadowed_v1.frag", &fs_src)?,
            terrain_fs: compile_glsl(
                ShaderStage::Fragment,
                "game_terrain_surface_v1.frag",
                &terrain_fs_src,
            )?,
            shadow_vs: compile_glsl(
                ShaderStage::Vertex,
                "game_sun_shadow_depth_v1.vert",
                &shadow_vs_src,
            )?,
            shadow_fs: compile_glsl(
                ShaderStage::Fragment,
                "game_sun_shadow_depth_v1.frag",
                &shadow_fs_src,
            )?,
            instanced_vs: compile_glsl(
                ShaderStage::Vertex,
                "game_lit_instanced_v1.vert",
                &instanced_vs_src,
            )?,
            instanced_fs: compile_glsl(
                ShaderStage::Fragment,
                "game_lit_instanced_v1.frag",
                &instanced_fs_src,
            )?,
            shadow_instanced_vs: compile_glsl(
                ShaderStage::Vertex,
                "game_sun_shadow_depth_instanced_v1.vert",
                &shadow_instanced_vs_src,
            )?,
        })
    }
}

fn load_text_asset(rel: &str) -> MaterialDomainResult<String> {
    let assets = AssetServiceClient::new(default_host_api());

    log::debug!("asset text: requesting path='{rel}' through AssetManager.text_v1");
    let payload = assets.text_v1(rel).map_err(|e| {
        MaterialDomainError::other(format!("asset.text_v1 failed path='{rel}' err='{e}'"))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| MaterialDomainError::other(format!("asset.text_v1 returned non-utf8 path='{rel}'")))?
        .to_string();

    log::debug!("asset text: loaded path='{rel}' bytes={}", payload.len());
    Ok(s)
}

fn compile_glsl(stage: ShaderStage, name: &str, src: &str) -> MaterialDomainResult<Vec<u32>> {
    newengine_shader_compiler::compile_glsl_to_spirv(stage, name, "main", src)
        .map_err(|e| MaterialDomainError::other(format!("shader compile failed: {e}")))
}
