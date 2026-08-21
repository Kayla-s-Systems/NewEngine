use std::time::Instant;

use newengine_material_domain_api::{
    LitPipeline, MaterialDomainError, MaterialDomainResult, MaterialPipelineBuildProfile,
    MaterialRenderDevice, LIT_INSTANCE_VERTEX_STRIDE,
};
use newengine_primitives::PrimitiveVertex;
use newengine_render_api::*;

use crate::manifest::{StandardLitShaderManifest, StandardShaderAssetRef};

const WARMUP_CPU_BUDGET_MS: f32 = 4.0;

#[inline]
const fn sky_depth_mode() -> PipelineDepthMode {
    PipelineDepthMode::new(true, false, PipelineDepthCompare::LessOrEqual)
}

/// Incremental Standard pipeline builder.
///
/// The old builder materialized every shader/resource/pipeline in one call. That
/// made a cold driver/cache path indistinguishable from a hung loading screen.
/// This state is retained by the provider and advances under the loading
/// projection. A warm cache may complete several cheap stages in one frame; a
/// slow stage yields immediately so the event loop can present the loading UI.
pub(super) struct PendingLitPipelineBuild {
    profile: MaterialPipelineBuildProfile,
    manifest: StandardLitShaderManifest,
    stage: u8,

    vs: Option<ShaderId>,
    fs: Option<ShaderId>,
    gbuffer_fs: Option<ShaderId>,
    gbuffer_terrain_fs: Option<ShaderId>,
    terrain_fs: Option<ShaderId>,
    shadow_vs: Option<ShaderId>,
    shadow_fs: Option<ShaderId>,
    instanced_vs: Option<ShaderId>,
    instanced_fs: Option<ShaderId>,
    shadow_instanced_vs: Option<ShaderId>,

    bgl: Option<BindGroupLayoutId>,
    white_texture: Option<TextureId>,
    flat_normal_texture: Option<TextureId>,
    repeat_sampler: Option<SamplerId>,
    clamp_sampler: Option<SamplerId>,

    pipeline: Option<PipelineId>,
    double_sided_pipeline: Option<PipelineId>,
    terrain_pipeline: Option<PipelineId>,
    gbuffer_terrain_pipeline: Option<PipelineId>,
    gbuffer_pipeline: Option<PipelineId>,
    gbuffer_double_sided_pipeline: Option<PipelineId>,
    shadow_pipeline: Option<PipelineId>,
    shadow_double_sided_pipeline: Option<PipelineId>,
    instanced_pipeline: Option<PipelineId>,
    instanced_double_sided_pipeline: Option<PipelineId>,
    gbuffer_instanced_pipeline: Option<PipelineId>,
    gbuffer_instanced_double_sided_pipeline: Option<PipelineId>,
    sky_instanced_pipeline: Option<PipelineId>,
    shadow_instanced_pipeline: Option<PipelineId>,
    shadow_instanced_double_sided_pipeline: Option<PipelineId>,
}

impl PendingLitPipelineBuild {
    pub(super) fn new(
        profile: MaterialPipelineBuildProfile,
        manifest: StandardLitShaderManifest,
    ) -> Self {
        Self {
            profile,
            manifest,
            stage: 0,
            vs: None,
            fs: None,
            gbuffer_fs: None,
            gbuffer_terrain_fs: None,
            terrain_fs: None,
            shadow_vs: None,
            shadow_fs: None,
            instanced_vs: None,
            instanced_fs: None,
            shadow_instanced_vs: None,
            bgl: None,
            white_texture: None,
            flat_normal_texture: None,
            repeat_sampler: None,
            clamp_sampler: None,
            pipeline: None,
            double_sided_pipeline: None,
            terrain_pipeline: None,
            gbuffer_terrain_pipeline: None,
            gbuffer_pipeline: None,
            gbuffer_double_sided_pipeline: None,
            shadow_pipeline: None,
            shadow_double_sided_pipeline: None,
            instanced_pipeline: None,
            instanced_double_sided_pipeline: None,
            gbuffer_instanced_pipeline: None,
            gbuffer_instanced_double_sided_pipeline: None,
            sky_instanced_pipeline: None,
            shadow_instanced_pipeline: None,
            shadow_instanced_double_sided_pipeline: None,
        }
    }

    pub(super) fn advance(
        &mut self,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<Option<LitPipeline>> {
        let frame_started = Instant::now();
        let start_stage = self.stage;
        let mut operations = 0_u32;

        loop {
            if self.stage >= 25 {
                let pipeline = self.finish()?;
                newengine_ulog_api::ulog::info!(
                    "standard material domain: staged pipeline ready stages={} operations_this_frame={} elapsed_ms={:.2} deferred_pipelines={}",
                    self.stage,
                    operations,
                    frame_started.elapsed().as_secs_f32() * 1000.0,
                    self.profile.deferred_pipelines,
                );
                return Ok(Some(pipeline));
            }

            let op_started = Instant::now();
            self.advance_one(r)?;
            operations = operations.saturating_add(1);
            let op_ms = op_started.elapsed().as_secs_f32() * 1000.0;
            if op_ms >= 8.0 {
                newengine_ulog_api::ulog::warn!(
                    "standard material domain: warmup stage slow stage={} elapsed_ms={:.2} deferred_pipelines={} action='yield_after_stage'",
                    self.stage.saturating_sub(1),
                    op_ms,
                    self.profile.deferred_pipelines,
                );
                break;
            }
            if frame_started.elapsed().as_secs_f32() * 1000.0 >= WARMUP_CPU_BUDGET_MS {
                break;
            }
        }

        newengine_ulog_api::ulog::debug!(
            "standard material domain: staged warmup progress stage={} previous_stage={} operations={} elapsed_ms={:.2} budget_ms={:.2}",
            self.stage,
            start_stage,
            operations,
            frame_started.elapsed().as_secs_f32() * 1000.0,
            WARMUP_CPU_BUDGET_MS,
        );
        Ok(None)
    }

    fn advance_one(&mut self, r: &mut dyn MaterialRenderDevice) -> MaterialDomainResult<()> {
        match self.stage {
            0 => {
                self.vs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Vertex,
                    &self.manifest.shaders.lit_vs,
                    "standard_lit_vs",
                )?)
            }
            1 => {
                self.fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.lit_fs,
                    "standard_lit_fs",
                )?)
            }
            2 if self.profile.deferred_pipelines => {
                self.gbuffer_fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.gbuffer_fs,
                    "standard_gbuffer_lit_fs",
                )?)
            }
            3 if self.profile.deferred_pipelines => {
                self.gbuffer_terrain_fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.gbuffer_terrain_fs,
                    "standard_gbuffer_terrain_fs",
                )?)
            }
            4 => {
                self.terrain_fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.terrain_fs,
                    "standard_terrain_surface_fs",
                )?)
            }
            5 => {
                self.shadow_vs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Vertex,
                    &self.manifest.shaders.shadow_vs,
                    "standard_sun_shadow_depth_vs",
                )?)
            }
            6 => {
                self.shadow_fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.shadow_fs,
                    "standard_sun_shadow_depth_fs",
                )?)
            }
            7 => {
                self.instanced_vs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Vertex,
                    &self.manifest.shaders.instanced_vs,
                    "standard_lit_instanced_vs",
                )?)
            }
            8 => {
                self.instanced_fs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Fragment,
                    &self.manifest.shaders.instanced_fs,
                    "standard_lit_instanced_fs",
                )?)
            }
            9 => {
                self.shadow_instanced_vs = Some(create_manifest_shader(
                    r,
                    ShaderStage::Vertex,
                    &self.manifest.shaders.shadow_instanced_vs,
                    "standard_sun_shadow_instanced_vs",
                )?)
            }
            10 => self.create_bind_resources(r)?,
            11 => {
                self.pipeline = Some(r.create_pipeline(self.pipeline_desc(false, false, false)?)?)
            }
            12 => {
                self.double_sided_pipeline =
                    Some(r.create_pipeline(self.pipeline_desc(true, false, false)?)?)
            }
            13 => self.terrain_pipeline = Some(r.create_pipeline(self.terrain_pipeline_desc()?)?),
            14 if self.profile.deferred_pipelines => {
                self.gbuffer_terrain_pipeline =
                    Some(r.create_pipeline(self.gbuffer_terrain_pipeline_desc()?)?)
            }
            15 if self.profile.deferred_pipelines => {
                self.gbuffer_pipeline =
                    Some(r.create_pipeline(self.gbuffer_pipeline_desc(false, false)?)?)
            }
            16 if self.profile.deferred_pipelines => {
                self.gbuffer_double_sided_pipeline =
                    Some(r.create_pipeline(self.gbuffer_pipeline_desc(true, false)?)?)
            }
            17 => {
                self.shadow_pipeline =
                    Some(r.create_pipeline(self.shadow_pipeline_desc(false, false)?)?)
            }
            18 => {
                self.shadow_double_sided_pipeline =
                    Some(r.create_pipeline(self.shadow_pipeline_desc(true, false)?)?)
            }
            19 => {
                self.instanced_pipeline =
                    Some(r.create_pipeline(self.pipeline_desc(false, true, false)?)?)
            }
            20 => {
                self.instanced_double_sided_pipeline =
                    Some(r.create_pipeline(self.pipeline_desc(true, true, false)?)?)
            }
            21 if self.profile.deferred_pipelines => {
                self.gbuffer_instanced_pipeline =
                    Some(r.create_pipeline(self.gbuffer_pipeline_desc(false, true)?)?)
            }
            22 if self.profile.deferred_pipelines => {
                self.gbuffer_instanced_double_sided_pipeline =
                    Some(r.create_pipeline(self.gbuffer_pipeline_desc(true, true)?)?)
            }
            23 => {
                self.sky_instanced_pipeline =
                    Some(r.create_pipeline(self.pipeline_desc(true, true, true)?)?)
            }
            24 => {
                self.shadow_instanced_pipeline =
                    Some(r.create_pipeline(self.shadow_pipeline_desc(false, true)?)?);
                self.shadow_instanced_double_sided_pipeline =
                    Some(r.create_pipeline(self.shadow_pipeline_desc(true, true)?)?);
            }
            _ => {}
        }
        self.stage = self.stage.saturating_add(1);
        Ok(())
    }

    fn create_bind_resources(
        &mut self,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<()> {
        self.bgl = Some(
            r.create_bind_group_layout(
                BindGroupLayoutDesc::new(vec![
                    BindingKind::UniformBuffer,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Texture2D,
                    BindingKind::Sampler,
                    // binding 6: local point/spot shadow atlas. Appended after the
                    // legacy sampler to preserve existing shader binding numbers.
                    BindingKind::Texture2D,
                ])
                .with_label("standard_lit_bgl"),
            )?,
        );
        self.white_texture = Some(
            r.create_texture(
                TextureDesc::new(
                    Extent2D::new(1, 1),
                    TextureFormat::Rgba8Unorm,
                    TextureUsage::Sampled,
                )
                .with_label("standard_white_tex")
                .with_data(vec![255, 255, 255, 255]),
            )?,
        );
        self.flat_normal_texture = Some(
            r.create_texture(
                TextureDesc::new(
                    Extent2D::new(1, 1),
                    TextureFormat::Rgba8Unorm,
                    TextureUsage::Sampled,
                )
                .with_label("standard_flat_normal_tex")
                .with_data(vec![128, 128, 255, 255]),
            )?,
        );
        self.repeat_sampler = Some(
            r.create_sampler(
                SamplerDesc::default()
                    .with_label("standard_repeat_sampler")
                    .with_repeat(),
            )?,
        );
        self.clamp_sampler = Some(
            r.create_sampler(
                SamplerDesc::default()
                    .with_label("standard_clamp_sampler")
                    .with_address_u(AddressMode::ClampToEdge)
                    .with_address_v(AddressMode::ClampToEdge)
                    .with_address_w(AddressMode::ClampToEdge),
            )?,
        );
        Ok(())
    }

    fn pipeline_desc(
        &self,
        double_sided: bool,
        instanced: bool,
        sky: bool,
    ) -> MaterialDomainResult<PipelineDesc> {
        let vs = if instanced {
            required(self.instanced_vs, "instanced_vs")?
        } else {
            required(self.vs, "vs")?
        };
        let fs = if instanced {
            required(self.instanced_fs, "instanced_fs")?
        } else {
            required(self.fs, "fs")?
        };
        let bgl = required(self.bgl, "bgl")?;
        let layouts = if instanced {
            vec![primitive_vertex_layout(), instance_vertex_layout()]
        } else {
            vec![primitive_vertex_layout()]
        };
        let label = match (double_sided, instanced, sky) {
            (_, true, true) => "standard_sky_pipeline_instanced",
            (true, true, false) => "standard_lit_pipeline_instanced_double_sided",
            (false, true, false) => "standard_lit_pipeline_instanced",
            (true, false, false) => "standard_lit_pipeline_double_sided",
            _ => "standard_lit_pipeline",
        };
        let mut desc = PipelineDesc::new(vs, fs, self.profile.scene_hdr_color_format)
            .with_label(label)
            .with_cache_key(format!(
                "standard:{label}:{:?}",
                self.profile.scene_hdr_color_format
            ))
            .as_warmup()
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(layouts)
            .with_bind_group_layouts(vec![bgl]);
        desc = if sky {
            // Sky is replayed after terrain/world opaque batches. It must remain
            // read-only in depth, but still test against the scene depth so the
            // dome only fills pixels where no nearer world geometry was drawn.
            desc.with_depth_state(TextureFormat::Depth32Float, sky_depth_mode())
        } else {
            desc.with_depth(TextureFormat::Depth32Float)
        };
        if double_sided || sky {
            desc = desc.with_cull_mode(RasterCullMode::None);
        }
        Ok(desc)
    }

    fn terrain_pipeline_desc(&self) -> MaterialDomainResult<PipelineDesc> {
        let label = "standard_terrain_surface_pipeline";
        Ok(PipelineDesc::new(
            required(self.vs, "vs")?,
            required(self.terrain_fs, "terrain_fs")?,
            self.profile.scene_hdr_color_format,
        )
        .with_label(label)
        .with_cache_key(format!(
            "standard:{label}:{:?}",
            self.profile.scene_hdr_color_format
        ))
        .as_warmup()
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(vec![primitive_vertex_layout()])
        .with_bind_group_layouts(vec![required(self.bgl, "bgl")?])
        .with_depth(TextureFormat::Depth32Float))
    }

    fn gbuffer_terrain_pipeline_desc(&self) -> MaterialDomainResult<PipelineDesc> {
        let label = "standard_gbuffer_terrain_pipeline";
        Ok(PipelineDesc::new(
            required(self.vs, "vs")?,
            required(self.gbuffer_terrain_fs, "gbuffer_terrain_fs")?,
            TextureFormat::Rgba8Unorm,
        )
        .with_label(label)
        .with_cache_key(format!("standard:{label}:gbuffer"))
        .as_warmup()
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(vec![primitive_vertex_layout()])
        .with_bind_group_layouts(vec![required(self.bgl, "bgl")?])
        .with_color_formats(gbuffer_color_formats())
        .with_depth(TextureFormat::Depth32Float))
    }

    fn gbuffer_pipeline_desc(
        &self,
        double_sided: bool,
        instanced: bool,
    ) -> MaterialDomainResult<PipelineDesc> {
        let label = match (double_sided, instanced) {
            (true, true) => "standard_gbuffer_lit_pipeline_instanced_double_sided",
            (false, true) => "standard_gbuffer_lit_pipeline_instanced",
            (true, false) => "standard_gbuffer_lit_pipeline_double_sided",
            _ => "standard_gbuffer_lit_pipeline",
        };
        let vs = if instanced {
            required(self.instanced_vs, "instanced_vs")?
        } else {
            required(self.vs, "vs")?
        };
        let layouts = if instanced {
            vec![primitive_vertex_layout(), instance_vertex_layout()]
        } else {
            vec![primitive_vertex_layout()]
        };
        let mut desc = PipelineDesc::new(
            vs,
            required(self.gbuffer_fs, "gbuffer_fs")?,
            TextureFormat::Rgba8Unorm,
        )
        .with_label(label)
        .with_cache_key(format!("standard:{label}:gbuffer"))
        .as_warmup()
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(layouts)
        .with_bind_group_layouts(vec![required(self.bgl, "bgl")?])
        .with_color_formats(gbuffer_color_formats())
        .with_depth(TextureFormat::Depth32Float);
        if double_sided {
            desc = desc.with_cull_mode(RasterCullMode::None);
        }
        Ok(desc)
    }

    fn shadow_pipeline_desc(
        &self,
        double_sided: bool,
        instanced: bool,
    ) -> MaterialDomainResult<PipelineDesc> {
        let label = match (double_sided, instanced) {
            (true, true) => "standard_sun_shadow_depth_pipeline_instanced_double_sided",
            (false, true) => "standard_sun_shadow_depth_pipeline_instanced",
            (true, false) => "standard_sun_shadow_depth_pipeline_double_sided",
            _ => "standard_sun_shadow_depth_pipeline",
        };
        let vs = if instanced {
            required(self.shadow_instanced_vs, "shadow_instanced_vs")?
        } else {
            required(self.shadow_vs, "shadow_vs")?
        };
        let layouts = if instanced {
            vec![primitive_vertex_layout(), instance_vertex_layout()]
        } else {
            vec![primitive_vertex_layout()]
        };
        let mut desc = PipelineDesc::new(
            vs,
            required(self.shadow_fs, "shadow_fs")?,
            self.profile.shadow_map_color_format,
        )
        .with_label(label)
        .with_cache_key(format!(
            "standard:{label}:{:?}",
            self.profile.shadow_map_color_format
        ))
        .as_warmup()
        .with_topology(PrimitiveTopology::TriangleList)
        .with_vertex_layouts(layouts)
        .with_bind_group_layouts(vec![required(self.bgl, "bgl")?])
        .with_depth(TextureFormat::Depth32Float);
        if double_sided {
            desc = desc.with_cull_mode(RasterCullMode::None);
        }
        Ok(desc)
    }

    fn finish(&self) -> MaterialDomainResult<LitPipeline> {
        let pipeline = required(self.pipeline, "pipeline")?;
        let double_sided_pipeline = required(self.double_sided_pipeline, "double_sided_pipeline")?;
        let terrain_pipeline = required(self.terrain_pipeline, "terrain_pipeline")?;
        let instanced_pipeline = required(self.instanced_pipeline, "instanced_pipeline")?;
        let instanced_double_sided_pipeline = required(
            self.instanced_double_sided_pipeline,
            "instanced_double_sided_pipeline",
        )?;
        Ok(LitPipeline {
            bgl: required(self.bgl, "bgl")?,
            white_texture: required(self.white_texture, "white_texture")?,
            flat_normal_texture: required(self.flat_normal_texture, "flat_normal_texture")?,
            repeat_sampler: required(self.repeat_sampler, "repeat_sampler")?,
            clamp_sampler: required(self.clamp_sampler, "clamp_sampler")?,
            vs: required(self.vs, "vs")?,
            fs: required(self.fs, "fs")?,
            terrain_fs: required(self.terrain_fs, "terrain_fs")?,
            shadow_vs: required(self.shadow_vs, "shadow_vs")?,
            shadow_fs: required(self.shadow_fs, "shadow_fs")?,
            pipeline,
            double_sided_pipeline,
            terrain_pipeline,
            gbuffer_terrain_pipeline: if self.profile.deferred_pipelines {
                required(self.gbuffer_terrain_pipeline, "gbuffer_terrain_pipeline")?
            } else {
                terrain_pipeline
            },
            gbuffer_pipeline: if self.profile.deferred_pipelines {
                required(self.gbuffer_pipeline, "gbuffer_pipeline")?
            } else {
                pipeline
            },
            gbuffer_double_sided_pipeline: if self.profile.deferred_pipelines {
                required(
                    self.gbuffer_double_sided_pipeline,
                    "gbuffer_double_sided_pipeline",
                )?
            } else {
                double_sided_pipeline
            },
            gbuffer_instanced_pipeline: if self.profile.deferred_pipelines {
                required(
                    self.gbuffer_instanced_pipeline,
                    "gbuffer_instanced_pipeline",
                )?
            } else {
                instanced_pipeline
            },
            gbuffer_instanced_double_sided_pipeline: if self.profile.deferred_pipelines {
                required(
                    self.gbuffer_instanced_double_sided_pipeline,
                    "gbuffer_instanced_double_sided_pipeline",
                )?
            } else {
                instanced_double_sided_pipeline
            },
            shadow_pipeline: required(self.shadow_pipeline, "shadow_pipeline")?,
            shadow_double_sided_pipeline: required(
                self.shadow_double_sided_pipeline,
                "shadow_double_sided_pipeline",
            )?,
            instanced_vs: required(self.instanced_vs, "instanced_vs")?,
            instanced_fs: required(self.instanced_fs, "instanced_fs")?,
            shadow_instanced_vs: required(self.shadow_instanced_vs, "shadow_instanced_vs")?,
            instanced_pipeline,
            instanced_double_sided_pipeline,
            sky_instanced_pipeline: required(
                self.sky_instanced_pipeline,
                "sky_instanced_pipeline",
            )?,
            shadow_instanced_pipeline: required(
                self.shadow_instanced_pipeline,
                "shadow_instanced_pipeline",
            )?,
            shadow_instanced_double_sided_pipeline: required(
                self.shadow_instanced_double_sided_pipeline,
                "shadow_instanced_double_sided_pipeline",
            )?,
        })
    }
}

#[inline]
fn required<T: Copy>(value: Option<T>, name: &str) -> MaterialDomainResult<T> {
    value.ok_or_else(|| {
        MaterialDomainError::other(format!("pipeline warmup invariant missing '{name}'"))
    })
}

fn primitive_vertex_layout() -> VertexLayout {
    let stride = std::mem::size_of::<PrimitiveVertex>() as u32;
    VertexLayout::new(
        stride,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(1, 12, VertexFormat::Float32x3),
            VertexAttribute::new(2, 24, VertexFormat::Float32x2),
        ],
    )
}

fn instance_vertex_layout() -> VertexLayout {
    VertexLayout::new(
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
    .per_instance()
}

fn gbuffer_color_formats() -> Vec<TextureFormat> {
    vec![
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba16Float,
        TextureFormat::Rgba8Unorm,
    ]
}

fn create_manifest_shader(
    r: &mut dyn MaterialRenderDevice,
    stage: ShaderStage,
    shader: &StandardShaderAssetRef,
    label: &str,
) -> MaterialDomainResult<ShaderId> {
    let source_kind = shader.source_kind()?;
    let started_at = Instant::now();
    let asset = ShaderAssetDesc::new(shader.logical_path.clone(), source_kind)
        .with_entry(shader.entry.clone())
        .with_variant(shader.variant_id.clone());
    let result = r.create_shader(
        ShaderDesc::from_asset(
            stage,
            shader.entry.clone(),
            shader.logical_path.clone(),
            source_kind,
        )
        .with_asset(asset)
        .with_label(label),
    );
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(id) if elapsed_ms >= 8.0 => newengine_ulog_api::ulog::warn!(
            "standard material domain: shader stage exceeded warmup budget label='{}' path='{}' stage='{:?}' shader_id={:?} elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            id,
            elapsed_ms,
        ),
        Err(error) => newengine_ulog_api::ulog::error!(
            "standard material domain: shader build failed label='{}' path='{}' stage='{:?}' err='{}' elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            error,
            elapsed_ms,
        ),
        _ => {}
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_depth_is_read_only_and_occlusion_aware() {
        let depth = sky_depth_mode();
        assert!(
            depth.test,
            "sky must respect depth written by world geometry"
        );
        assert!(!depth.write, "sky must never overwrite scene depth");
        assert_eq!(depth.compare, PipelineDepthCompare::LessOrEqual);
    }
}
