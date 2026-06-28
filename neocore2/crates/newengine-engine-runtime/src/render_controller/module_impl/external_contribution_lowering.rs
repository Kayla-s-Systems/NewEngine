#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BufferSlice, DrawArgs, DrawIndexedArgs, DrawListContribution, DrawListContributionCommand,
    RenderApi, RenderDrawListKind, RenderInstanceSource, RenderMaterialBinding,
    RenderMeshGpuBinding, RenderPipelineClass,
};
use newengine_core::EngineResult;
use newengine_math::{Mat4, Vec3, Vec4};
use newengine_render_feature_api::SceneExtractionCtx;

use super::super::gpu::ensure_debug_line_pipeline;
use super::draw_lists::{DrawListBuildCtx, ExternalRenderDrawListProviderDesc};
use super::RuntimeRenderController;

#[derive(Clone, Copy, Debug)]
pub(super) struct ExternalContributionLoweringReport {
    pub(super) draw_list: RenderDrawListKind,
    pub(super) commands: usize,
    pub(super) draw_calls: u32,
    pub(super) skipped_commands: u32,
    pub(super) triangle_count: u64,
}

impl Default for ExternalContributionLoweringReport {
    #[inline]
    fn default() -> Self {
        Self {
            draw_list: RenderDrawListKind::OpaqueForward,
            commands: 0,
            draw_calls: 0,
            skipped_commands: 0,
            triangle_count: 0,
        }
    }
}

pub(super) fn lower_external_draw_list_contribution(
    provider: &ExternalRenderDrawListProviderDesc,
    contribution: DrawListContribution,
    ctx: &SceneExtractionCtx<'_>,
    out: &mut DrawListBuildCtx<'_>,
) -> EngineResult<ExternalContributionLoweringReport> {
    for warning in &contribution.warnings {
        newengine_ulog_api::ulog::warn!(
            "render draw-list provider '{}' contribution '{}': {}",
            provider.id,
            contribution.label,
            warning
        );
    }

    let mut report = ExternalContributionLoweringReport {
        draw_list: contribution.draw_list,
        commands: contribution.commands.len(),
        triangle_count: contribution.stats.triangle_count,
        ..ExternalContributionLoweringReport::default()
    };

    for command in contribution.commands {
        match command {
            DrawListContributionCommand::GpuMesh {
                mesh,
                material,
                material_binding,
                gpu,
                instances,
                pipeline,
            } => {
                let lowered = lower_gpu_mesh_contribution(
                    provider,
                    contribution.draw_list,
                    mesh.stable_label(),
                    material.map(|it| it.stable_label()),
                    material_binding,
                    gpu,
                    instances,
                    pipeline,
                    ctx,
                    out,
                )?;
                if lowered {
                    report.draw_calls = report.draw_calls.saturating_add(1);
                } else {
                    report.skipped_commands = report.skipped_commands.saturating_add(1);
                }
            }
            DrawListContributionCommand::DebugLineList { vertices, color } => {
                let lowered = lower_debug_line_list_contribution(
                    provider,
                    contribution.draw_list,
                    vertices,
                    color,
                    ctx,
                    out,
                )?;
                if lowered {
                    report.draw_calls = report.draw_calls.saturating_add(1);
                } else {
                    report.skipped_commands = report.skipped_commands.saturating_add(1);
                }
            }
        }
    }

    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn lower_gpu_mesh_contribution(
    provider: &ExternalRenderDrawListProviderDesc,
    draw_list: RenderDrawListKind,
    mesh_label: String,
    material_label: Option<String>,
    material_binding: RenderMaterialBinding,
    gpu: RenderMeshGpuBinding,
    instances: RenderInstanceSource,
    pipeline: RenderPipelineClass,
    ctx: &SceneExtractionCtx<'_>,
    out: &mut DrawListBuildCtx<'_>,
) -> EngineResult<bool> {
    if draw_list == RenderDrawListKind::ShadowCasters && !material_binding.cast_shadows {
        return Ok(false);
    }

    match instances {
        RenderInstanceSource::Inline(items) => {
            let instances = if items.is_empty() {
                vec![RenderInstanceForLowering {
                    transform: Mat4::IDENTITY,
                    base_color_override: None,
                }]
            } else {
                items
                    .into_iter()
                    .map(|it| RenderInstanceForLowering {
                        transform: Mat4::from_cols_array_2d(&it.transform_cols),
                        base_color_override: it.base_color_override,
                    })
                    .collect()
            };

            let key_base = stable_u64(&provider.plugin_id)
                ^ stable_u64(&provider.id).rotate_left(7)
                ^ stable_u64(&mesh_label).rotate_left(17)
                ^ material_label
                    .as_deref()
                    .map(stable_u64)
                    .unwrap_or(0)
                    .rotate_left(29);

            let _ = out.record(draw_list, move |this, r| {
                for (idx, instance) in instances.iter().enumerate() {
                    lower_single_gpu_mesh_instance(
                        this,
                        r,
                        draw_list,
                        ctx,
                        key_base ^ ((idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
                        material_binding.clone(),
                        gpu,
                        instance.transform,
                        instance.base_color_override,
                        pipeline,
                    )?;
                }
                Ok(())
            })?;
            Ok(true)
        }
        RenderInstanceSource::Buffer(binding) => {
            newengine_ulog_api::ulog::warn!(
                "render draw-list provider '{}' mesh='{}' supplied instance buffer '{}'; skipped because lit shader instance-buffer vertex layout is not enabled yet",
                provider.id,
                mesh_label,
                binding.handle.stable_label()
            );
            Ok(false)
        }
    }
}

fn lower_debug_line_list_contribution(
    provider: &ExternalRenderDrawListProviderDesc,
    draw_list: RenderDrawListKind,
    vertices: Vec<[f32; 3]>,
    color: [f32; 4],
    ctx: &SceneExtractionCtx<'_>,
    out: &mut DrawListBuildCtx<'_>,
) -> EngineResult<bool> {
    if vertices.len() < 2 {
        return Ok(false);
    }

    if !matches!(
        draw_list,
        RenderDrawListKind::Debug | RenderDrawListKind::OpaqueForward
    ) {
        newengine_ulog_api::ulog::warn!(
            "render draw-list provider '{}' submitted DebugLineList to mismatched draw-list '{}'",
            provider.id,
            draw_list.label()
        );
        return Ok(false);
    }

    let _ = out.record(draw_list, move |this, r| {
        let pair_count = vertices.len() / 2;
        let vertex_count = (pair_count * 2) as u32;
        if vertex_count == 0 {
            return Ok(());
        }

        let mut bytes = Vec::with_capacity(vertex_count as usize * 32);
        for pair in vertices.chunks_exact(2) {
            push_debug_line_vertex(&mut bytes, ctx.viewproj, pair[0], color);
            push_debug_line_vertex(&mut bytes, ctx.viewproj, pair[1], color);
        }

        let gpu =
            ensure_debug_line_pipeline(&mut this.gpu.meshes.collision_lines, r, vertex_count)?;
        r.write_buffer(gpu.vb, 0, &bytes)?;
        r.set_pipeline(gpu.pipeline)?;
        r.set_bind_group(0, gpu.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.draw(DrawArgs::new(vertex_count))?;
        Ok(())
    })?;
    Ok(true)
}

fn push_debug_line_vertex(
    bytes: &mut Vec<u8>,
    viewproj: Mat4,
    position_ws: [f32; 3],
    color: [f32; 4],
) {
    let p = Vec3::new(position_ws[0], position_ws[1], position_ws[2]);
    let clip = viewproj * Vec4::new(p.x, p.y, p.z, 1.0);
    for value in [
        clip.x, clip.y, clip.z, clip.w, color[0], color[1], color[2], color[3],
    ] {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
}

#[derive(Clone, Copy)]
struct RenderInstanceForLowering {
    transform: Mat4,
    base_color_override: Option<[f32; 4]>,
}

#[allow(clippy::too_many_arguments)]
fn lower_single_gpu_mesh_instance(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    draw_list: RenderDrawListKind,
    ctx: &SceneExtractionCtx<'_>,
    key: u64,
    material: RenderMaterialBinding,
    gpu: RenderMeshGpuBinding,
    model: Mat4,
    base_color_override: Option<[f32; 4]>,
    pipeline: RenderPipelineClass,
) -> EngineResult<()> {
    let lit = ctx.lit;
    let base_tex = this.material_texture_or_default(
        r,
        material.base_color_texture.as_deref(),
        lit.white_texture,
    );
    let normal_tex = this.material_texture_or_default(
        r,
        material.normal_texture.as_deref(),
        lit.flat_normal_texture,
    );
    let roughness_tex = this.material_texture_or_default(
        r,
        material.roughness_texture.as_deref(),
        lit.white_texture,
    );
    let sampler = if material.base_color_texture.is_some()
        || material.normal_texture.is_some()
        || material.roughness_texture.is_some()
    {
        lit.repeat_sampler
    } else {
        lit.clamp_sampler
    };

    if matches!(draw_list, RenderDrawListKind::ShadowCasters) {
        let (center_ws, radius_ws) = conservative_model_sphere(model);
        if !ctx
            .shadow_plan
            .caster_cull
            .map(|c| c.contains_sphere(center_ws, radius_ws))
            .unwrap_or(true)
        {
            return Ok(());
        }
    }

    let shadow_texture =
        if matches!(draw_list, RenderDrawListKind::ShadowCasters) || !material.receive_shadows {
            lit.white_texture
        } else {
            ctx.shadow_frame.texture
        };

    let mut per = this.ensure_per_draw_ubo_with_binding(
        r,
        lit,
        key,
        base_tex,
        normal_tex,
        roughness_tex,
        shadow_texture,
        sampler,
    )?;
    per.last_seen_frame = this.frame.frame_index;
    this.gpu.material.per_draw_ubo.insert(key, per);

    let mvp = if matches!(draw_list, RenderDrawListKind::ShadowCasters) {
        ctx.shadow_frame.light_mvp * model
    } else {
        ctx.viewproj * model
    };
    let base_color = base_color_override.unwrap_or(material.base_color);
    super::passes_ubo::write_lit_ubo_ex(
        r,
        per.ubo,
        mvp,
        model,
        base_color,
        material.emissive_radiance,
        material.uv_transform,
        material.material_params,
        &ctx.lights,
    )?;

    let selected_pipeline = select_pipeline(ctx, draw_list, pipeline, material.double_sided);
    r.set_pipeline(selected_pipeline)?;
    r.set_bind_group(0, per.bg)?;
    r.set_vertex_buffer(0, gpu.vertex)?;
    if let Some(index) = gpu.index {
        r.set_index_buffer(index, gpu.index_format)?;
        r.draw_indexed(DrawIndexedArgs {
            index_count: gpu.index_count.max(1),
            instance_count: 1,
            first_index: gpu.first_index,
            vertex_offset: gpu.vertex_offset,
            first_instance: 0,
        })?;
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(gpu.index_count);
    } else {
        r.draw(DrawArgs {
            vertex_count: gpu.vertex_count.max(1),
            instance_count: 1,
            first_vertex: gpu.first_vertex,
            first_instance: 0,
        })?;
        this.diagnostics
            .overlay_metrics
            .record_vertices_as_triangles(gpu.vertex_count);
    }
    Ok(())
}

fn select_pipeline(
    ctx: &SceneExtractionCtx<'_>,
    draw_list: RenderDrawListKind,
    requested: RenderPipelineClass,
    double_sided: bool,
) -> newengine_core::render::PipelineId {
    let lit = ctx.lit;
    match (draw_list, requested) {
        (RenderDrawListKind::ShadowCasters, _) | (_, RenderPipelineClass::ShadowDepth) => {
            if double_sided {
                lit.shadow_double_sided_pipeline
            } else {
                lit.shadow_pipeline
            }
        }
        (_, RenderPipelineClass::TransparentForward)
        | (_, RenderPipelineClass::LitForward)
        | (_, RenderPipelineClass::DebugLines) => {
            if double_sided {
                lit.double_sided_pipeline
            } else {
                lit.pipeline
            }
        }
    }
}

fn stable_u64(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[inline]
fn conservative_model_sphere(model: Mat4) -> (Vec3, f32) {
    let center = model.transform_point3(Vec3::ZERO);
    let sx = model.x_axis.truncate().length();
    let sy = model.y_axis.truncate().length();
    let sz = model.z_axis.truncate().length();
    (center, sx.max(sy).max(sz).max(0.001) * 1.75)
}
