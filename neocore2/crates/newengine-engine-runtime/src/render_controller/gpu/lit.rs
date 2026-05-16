use crate::render_controller::render_quality::{SCENE_HDR_COLOR_FORMAT, SHADOW_MAP_COLOR_FORMAT};
use newengine_core::render::*;
use newengine_core::EngineResult as CoreResult;
use newengine_primitives::PrimitiveVertex;

use super::super::module_impl::instancing::RenderInstanceRaw;

use super::shader_assets::{compile_glsl, load_text_asset};
use super::types::LitPipeline;

pub fn ensure_lit_pipeline(
    cached: &mut Option<LitPipeline>,
    r: &mut dyn newengine_core::render::RenderApi,
) -> CoreResult<LitPipeline> {
    if let Some(p) = *cached {
        return Ok(p);
    }
    let vs_src = load_text_asset("shaders/game_lit_shadowed_v1.vert")?;
    let fs_src = load_text_asset("shaders/game_lit_shadowed_v1.frag")?;
    let terrain_fs_src = load_text_asset("shaders/game_terrain_surface_v1.frag")?;
    let shadow_vs_src = load_text_asset("shaders/game_sun_shadow_depth_v1.vert")?;
    let shadow_fs_src = load_text_asset("shaders/game_sun_shadow_depth_v1.frag")?;
    let instanced_vs_src = load_text_asset("shaders/game_lit_instanced_v1.vert")?;
    let instanced_fs_src = load_text_asset("shaders/game_lit_instanced_v1.frag")?;
    let shadow_instanced_vs_src = load_text_asset("shaders/game_sun_shadow_depth_instanced_v1.vert")?;

    let vs_spv = compile_glsl(ShaderStage::Vertex, "game_lit_shadowed_v1.vert", &vs_src)?;
    let fs_spv = compile_glsl(ShaderStage::Fragment, "game_lit_shadowed_v1.frag", &fs_src)?;
    let terrain_fs_spv = compile_glsl(ShaderStage::Fragment, "game_terrain_surface_v1.frag", &terrain_fs_src)?;
    let shadow_vs_spv = compile_glsl(ShaderStage::Vertex, "game_sun_shadow_depth_v1.vert", &shadow_vs_src)?;
    let shadow_fs_spv = compile_glsl(ShaderStage::Fragment, "game_sun_shadow_depth_v1.frag", &shadow_fs_src)?;
    let instanced_vs_spv = compile_glsl(ShaderStage::Vertex, "game_lit_instanced_v1.vert", &instanced_vs_src)?;
    let instanced_fs_spv = compile_glsl(ShaderStage::Fragment, "game_lit_instanced_v1.frag", &instanced_fs_src)?;
    let shadow_instanced_vs_spv = compile_glsl(ShaderStage::Vertex, "game_sun_shadow_depth_instanced_v1.vert", &shadow_instanced_vs_src)?;

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
        .with_label("game_lit_bgl"),
    )?;
    let white_texture = r.create_texture(
        TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
            .with_label("game_white_tex")
            .with_data(vec![255, 255, 255, 255]),
    )?;
    let flat_normal_texture = r.create_texture(
        TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
            .with_label("game_flat_normal_tex")
            .with_data(vec![128, 128, 255, 255]),
    )?;
    let repeat_sampler = r.create_sampler(
        SamplerDesc::default()
            .with_label("game_repeat_sampler")
            .with_repeat(),
    )?;
    let clamp_sampler = r.create_sampler(
        SamplerDesc::default()
            .with_label("game_clamp_sampler")
            .with_address_u(AddressMode::ClampToEdge)
            .with_address_v(AddressMode::ClampToEdge)
            .with_address_w(AddressMode::ClampToEdge),
    )?;
    let vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("game_lit_vs"),
    )?;
    let fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("game_lit_fs"),
    )?;
    let terrain_fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", terrain_fs_spv).with_label("game_terrain_surface_fs"),
    )?;
    let shadow_vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", shadow_vs_spv).with_label("game_sun_shadow_depth_vs"),
    )?;
    let shadow_fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", shadow_fs_spv).with_label("game_sun_shadow_depth_fs"),
    )?;
    let instanced_vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", instanced_vs_spv).with_label("game_lit_instanced_vs"),
    )?;
    let instanced_fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", instanced_fs_spv).with_label("game_lit_instanced_fs"),
    )?;
    let shadow_instanced_vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", shadow_instanced_vs_spv).with_label("game_sun_shadow_instanced_vs"),
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
        RenderInstanceRaw::stride(),
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
        PipelineDesc::new(vs, fs, SCENE_HDR_COLOR_FORMAT)
            .with_label("game_lit_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, SCENE_HDR_COLOR_FORMAT)
            .with_label("game_lit_pipeline_double_sided")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float)
            .with_cull_mode(RasterCullMode::None),
    )?;

    let terrain_pipeline = r.create_pipeline(
        PipelineDesc::new(vs, terrain_fs, SCENE_HDR_COLOR_FORMAT)
            .with_label("game_terrain_surface_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let shadow_pipeline = r.create_pipeline(
        PipelineDesc::new(
            shadow_vs,
            shadow_fs,
            SHADOW_MAP_COLOR_FORMAT,
        )
        .with_label("game_sun_shadow_depth_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let shadow_double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(
            shadow_vs,
            shadow_fs,
            SHADOW_MAP_COLOR_FORMAT,
        )
        .with_label("game_sun_shadow_depth_pipeline_double_sided")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float)
            .with_cull_mode(RasterCullMode::None),
    )?;

    let instanced_layouts = vec![layout.clone(), instance_layout.clone()];
    let instanced_pipeline = r.create_pipeline(
        PipelineDesc::new(instanced_vs, instanced_fs, SCENE_HDR_COLOR_FORMAT)
            .with_label("game_lit_pipeline_instanced")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(instanced_layouts.clone())
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let instanced_double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(instanced_vs, instanced_fs, SCENE_HDR_COLOR_FORMAT)
            .with_label("game_lit_pipeline_instanced_double_sided")
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
            SHADOW_MAP_COLOR_FORMAT,
        )
        .with_label("game_sun_shadow_depth_pipeline_instanced")
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(instanced_layouts.clone())
        .with_bind_group_layouts(vec![bgl])
        .with_depth(TextureFormat::Depth32Float),
    )?;

    let shadow_instanced_double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(
            shadow_instanced_vs,
            shadow_fs,
            SHADOW_MAP_COLOR_FORMAT,
        )
        .with_label("game_sun_shadow_depth_pipeline_instanced_double_sided")
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(instanced_layouts)
        .with_bind_group_layouts(vec![bgl])
        .with_depth(TextureFormat::Depth32Float)
        .with_cull_mode(RasterCullMode::None),
    )?;

    let p = LitPipeline {
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
    };

    *cached = Some(p);
    Ok(p)
}
