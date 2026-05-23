#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady/FPS runtime-lit material-domain provider.
//!
//! This crate owns the GameReady material-domain preset, not shader compilation.
//! Shader source paths are declared in a data asset and are compiled/validated on
//! demand by the active renderer backend through `ShaderDesc::from_asset`.

use std::time::Instant;

use newengine_assets::AssetServiceClient;
use newengine_material_domain_api::{
    LitPipeline, MaterialDomainError, MaterialDomainResult, MaterialGpuPipeline,
    MaterialGpuPipelineKey, MaterialGpuPipelineProvider, MaterialPipelineBuildProfile,
    MaterialRenderDevice, LIT_INSTANCE_VERTEX_STRIDE,
};
use newengine_plugin_host::default_host_api;
use newengine_primitives::PrimitiveVertex;
use newengine_render_api::*;
use serde::Deserialize;

pub const GAME_READY_LIT_PIPELINE_KEY: MaterialGpuPipelineKey =
    MaterialGpuPipelineKey::new("newengine.material_domain.gameready.runtime_lit");

const DEFAULT_SHADER_MANIFEST_PATH: &str = "shaders/pipelines/gameready_lit.pipeline.json";

#[derive(Default)]
pub struct GameReadyLitMaterialDomainProvider {
    manifest: Option<GameReadyLitShaderManifest>,
    pipeline: Option<LitPipeline>,
}

impl GameReadyLitMaterialDomainProvider {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn require_manifest(&mut self) -> MaterialDomainResult<GameReadyLitShaderManifest> {
        if let Some(manifest) = self.manifest.clone() {
            return Ok(manifest);
        }

        let manifest = GameReadyLitShaderManifest::load(DEFAULT_SHADER_MANIFEST_PATH)?;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    fn build_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<LitPipeline> {
        let started_at = Instant::now();
        log::info!(
            "gameready material domain: pipeline build requested key='{}' manifest='{}'",
            GAME_READY_LIT_PIPELINE_KEY.as_str(),
            DEFAULT_SHADER_MANIFEST_PATH
        );
        let manifest = self.require_manifest()?;
        log::info!(
            "gameready material domain: creating bind resources key='{}'",
            GAME_READY_LIT_PIPELINE_KEY.as_str()
        );

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
        log::info!(
            "gameready material domain: bind resources created key='{}' elapsed_ms={:.2}",
            GAME_READY_LIT_PIPELINE_KEY.as_str(),
            started_at.elapsed().as_secs_f64() * 1000.0
        );

        log::info!(
            "gameready material domain: requesting renderer-owned shader builds key='{}' shader_count=8",
            GAME_READY_LIT_PIPELINE_KEY.as_str()
        );
        let vs = create_manifest_shader(r, ShaderStage::Vertex, &manifest.shaders.lit_vs, "gameready_lit_vs")?;
        let fs = create_manifest_shader(r, ShaderStage::Fragment, &manifest.shaders.lit_fs, "gameready_lit_fs")?;
        let terrain_fs = create_manifest_shader(
            r,
            ShaderStage::Fragment,
            &manifest.shaders.terrain_fs,
            "gameready_terrain_surface_fs",
        )?;
        let shadow_vs = create_manifest_shader(
            r,
            ShaderStage::Vertex,
            &manifest.shaders.shadow_vs,
            "gameready_sun_shadow_depth_vs",
        )?;
        let shadow_fs = create_manifest_shader(
            r,
            ShaderStage::Fragment,
            &manifest.shaders.shadow_fs,
            "gameready_sun_shadow_depth_fs",
        )?;
        let instanced_vs = create_manifest_shader(
            r,
            ShaderStage::Vertex,
            &manifest.shaders.instanced_vs,
            "gameready_lit_instanced_vs",
        )?;
        let instanced_fs = create_manifest_shader(
            r,
            ShaderStage::Fragment,
            &manifest.shaders.instanced_fs,
            "gameready_lit_instanced_fs",
        )?;
        let shadow_instanced_vs = create_manifest_shader(
            r,
            ShaderStage::Vertex,
            &manifest.shaders.shadow_instanced_vs,
            "gameready_sun_shadow_instanced_vs",
        )?;
        log::info!(
            "gameready material domain: renderer-owned shader handles ready key='{}' elapsed_ms={:.2}",
            GAME_READY_LIT_PIPELINE_KEY.as_str(),
            started_at.elapsed().as_secs_f64() * 1000.0
        );

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

        log::info!(
            "gameready material domain: creating GPU pipelines key='{}' pipeline_count=9",
            GAME_READY_LIT_PIPELINE_KEY.as_str()
        );
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

        let sky_instanced_pipeline = r.create_pipeline(
            PipelineDesc::new(instanced_vs, instanced_fs, profile.scene_hdr_color_format)
                .with_label("gameready_sky_pipeline_instanced")
                .with_topology(PrimitiveTopology::TriangleList)
                .with_vertex_layouts(instanced_layouts.clone())
                .with_bind_group_layouts(vec![bgl])
                .with_depth_state(TextureFormat::Depth32Float, PipelineDepthMode::no_write_always())
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
        log::info!(
            "gameready material domain: pipeline build completed key='{}' elapsed_ms={:.2}",
            GAME_READY_LIT_PIPELINE_KEY.as_str(),
            started_at.elapsed().as_secs_f64() * 1000.0
        );

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
            sky_instanced_pipeline,
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

#[derive(Clone, Deserialize)]
struct GameReadyLitShaderManifest {
    #[allow(dead_code)]
    schema: String,
    shaders: GameReadyLitShaderSetManifest,
}

impl GameReadyLitShaderManifest {
    fn load(logical_path: &str) -> MaterialDomainResult<Self> {
        let source = load_text_asset(logical_path)?;
        let manifest: Self = serde_json::from_str(&source).map_err(|e| {
            MaterialDomainError::other(format!(
                "GameReady shader manifest parse failed path='{logical_path}' err='{e}'"
            ))
        })?;
        manifest.validate(logical_path)?;
        log::info!(
            "gameready material domain: shader manifest loaded path='{}' schema='{}'",
            logical_path,
            manifest.schema
        );
        Ok(manifest)
    }

    fn validate(&self, logical_path: &str) -> MaterialDomainResult<()> {
        if self.schema.trim().is_empty() {
            return Err(MaterialDomainError::other(format!(
                "GameReady shader manifest path='{logical_path}' missing schema"
            )));
        }
        self.shaders.validate(logical_path)
    }
}

#[derive(Clone, Deserialize)]
struct GameReadyLitShaderSetManifest {
    lit_vs: GameReadyShaderAssetRef,
    lit_fs: GameReadyShaderAssetRef,
    terrain_fs: GameReadyShaderAssetRef,
    shadow_vs: GameReadyShaderAssetRef,
    shadow_fs: GameReadyShaderAssetRef,
    instanced_vs: GameReadyShaderAssetRef,
    instanced_fs: GameReadyShaderAssetRef,
    shadow_instanced_vs: GameReadyShaderAssetRef,
}

impl GameReadyLitShaderSetManifest {
    fn validate(&self, manifest_path: &str) -> MaterialDomainResult<()> {
        for (field, shader) in [
            ("lit_vs", &self.lit_vs),
            ("lit_fs", &self.lit_fs),
            ("terrain_fs", &self.terrain_fs),
            ("shadow_vs", &self.shadow_vs),
            ("shadow_fs", &self.shadow_fs),
            ("instanced_vs", &self.instanced_vs),
            ("instanced_fs", &self.instanced_fs),
            ("shadow_instanced_vs", &self.shadow_instanced_vs),
        ] {
            shader.validate(manifest_path, field)?;
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize)]
struct GameReadyShaderAssetRef {
    logical_path: String,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    #[serde(default = "default_entry")]
    entry: String,
    #[serde(default = "default_variant")]
    variant_id: String,
}

impl GameReadyShaderAssetRef {
    fn validate(&self, manifest_path: &str, field: &str) -> MaterialDomainResult<()> {
        if self.logical_path.trim().is_empty() {
            return Err(MaterialDomainError::other(format!(
                "GameReady shader manifest path='{manifest_path}' field='{field}' has empty logical_path"
            )));
        }
        let _ = self.source_kind()?;
        Ok(())
    }

    fn source_kind(&self) -> MaterialDomainResult<ShaderSourceKind> {
        match self.source_kind.trim().to_ascii_lowercase().as_str() {
            "glsl" => Ok(ShaderSourceKind::Glsl),
            "hlsl" => Ok(ShaderSourceKind::Hlsl),
            "wgsl" => Ok(ShaderSourceKind::Wgsl),
            "spirv" | "spv" => Ok(ShaderSourceKind::Spirv),
            other => Err(MaterialDomainError::other(format!(
                "unsupported shader source_kind='{other}' path='{}'",
                self.logical_path
            ))),
        }
    }
}

fn default_source_kind() -> String {
    "glsl".to_owned()
}

fn default_entry() -> String {
    "main".to_owned()
}

fn default_variant() -> String {
    "gameready_default".to_owned()
}

fn create_manifest_shader(
    r: &mut dyn MaterialRenderDevice,
    stage: ShaderStage,
    shader: &GameReadyShaderAssetRef,
    label: &str,
) -> MaterialDomainResult<ShaderId> {
    let source_kind = shader.source_kind()?;
    let started_at = Instant::now();
    log::info!(
        "gameready material domain: shader build request label='{}' path='{}' stage='{:?}' source_kind='{}' entry='{}' variant='{}'",
        label,
        shader.logical_path,
        stage,
        source_kind.label(),
        shader.entry,
        shader.variant_id
    );
    let asset = ShaderAssetDesc::new(shader.logical_path.clone(), source_kind)
        .with_entry(shader.entry.clone())
        .with_variant(shader.variant_id.clone());
    let result = r.create_shader(
        ShaderDesc::from_asset(stage, shader.entry.clone(), shader.logical_path.clone(), source_kind)
            .with_asset(asset)
            .with_label(label),
    );
    match &result {
        Ok(id) => log::info!(
            "gameready material domain: shader build accepted label='{}' path='{}' stage='{:?}' shader_id={:?} elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            id,
            started_at.elapsed().as_secs_f64() * 1000.0
        ),
        Err(e) => log::error!(
            "gameready material domain: shader build failed label='{}' path='{}' stage='{:?}' err='{}' elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            e,
            started_at.elapsed().as_secs_f64() * 1000.0
        ),
    }
    result
}

fn load_text_asset(rel: &str) -> MaterialDomainResult<String> {
    let assets = AssetServiceClient::new(default_host_api());

    log::trace!("asset text: requesting path='{rel}' through AssetManager.text_v1");
    let payload = assets.text_v1(rel).map_err(|e| {
        MaterialDomainError::other(format!("asset.text_v1 failed path='{rel}' err='{e}'"))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| MaterialDomainError::other(format!("asset.text_v1 returned non-utf8 path='{rel}'")))?
        .to_string();

    log::trace!("asset text: loaded path='{rel}' bytes={}", payload.len());
    Ok(s)
}
