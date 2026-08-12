use super::*;

#[inline]
fn asset_preview_model_scale(extent: Vec3) -> f32 {
    2.2 / extent.x.max(extent.y).max(extent.z).max(0.001)
}

#[inline]
fn asset_preview_model_transform(center: Vec3, extent: Vec3, angle_radians: f32) -> Mat4 {
    let scale = asset_preview_model_scale(extent);
    let rotation = newengine_math::Quat::from_rotation_y(angle_radians);

    // Center in model space first, then scale and rotate. Encoding the centering
    // as the final world translation (`-center * scale`) is incorrect for a
    // rotated asset: the rotation moves the scaled source center away from the
    // origin, leaving only a thin edge or an empty preview for off-origin meshes.
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, Vec3::ZERO)
        * Mat4::from_translation(-center)
}

const PREVIEW_CAMERA_MIN_PITCH: f32 = -1.30;
const PREVIEW_CAMERA_MAX_PITCH: f32 = 1.30;
const PREVIEW_CAMERA_MIN_DISTANCE: f32 = 1.65;
const PREVIEW_CAMERA_MAX_DISTANCE: f32 = 12.0;

#[inline]
fn asset_preview_camera_position(view: newengine_render_feature_api::AssetPreviewView) -> Vec3 {
    let pitch = view
        .pitch_radians
        .clamp(PREVIEW_CAMERA_MIN_PITCH, PREVIEW_CAMERA_MAX_PITCH);
    let distance = view
        .distance
        .clamp(PREVIEW_CAMERA_MIN_DISTANCE, PREVIEW_CAMERA_MAX_DISTANCE);
    let horizontal = pitch.cos() * distance;
    Vec3::new(
        view.yaw_radians.sin() * horizontal,
        pitch.sin() * distance,
        view.yaw_radians.cos() * horizontal,
    )
}

const PREVIEW_GRID_HALF_EXTENT: f32 = 4.0;
const PREVIEW_GRID_MINOR_STEP: f32 = 0.25;
const PREVIEW_GRID_MAJOR_STEP: f32 = 1.0;

fn push_asset_preview_grid_quad(
    vertices: &mut Vec<newengine_primitives::PrimitiveVertex>,
    indices: &mut Vec<u32>,
    along_x: bool,
    offset: f32,
    half_width: f32,
) {
    let base = vertices.len() as u32;
    let e = PREVIEW_GRID_HALF_EXTENT;
    let (p0, p1, p2, p3) = if along_x {
        (
            [-e, 0.0, offset - half_width],
            [e, 0.0, offset - half_width],
            [e, 0.0, offset + half_width],
            [-e, 0.0, offset + half_width],
        )
    } else {
        (
            [offset - half_width, 0.0, -e],
            [offset + half_width, 0.0, -e],
            [offset + half_width, 0.0, e],
            [offset - half_width, 0.0, e],
        )
    };
    for pos in [p0, p1, p2, p3] {
        vertices.push(newengine_primitives::PrimitiveVertex {
            pos,
            nrm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });
    }
    // Double-sided preview pipeline makes winding irrelevant, but keep the
    // authored normal facing +Y for correct editor-preview illumination.
    indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
}

fn asset_preview_grid_mesh(
    step: f32,
    half_width: f32,
    omit_major_lines: bool,
) -> newengine_primitives::PrimitiveMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let count = (PREVIEW_GRID_HALF_EXTENT / step).round() as i32;
    let major_every = (PREVIEW_GRID_MAJOR_STEP / step).round().max(1.0) as i32;
    for index in -count..=count {
        if index == 0 || (omit_major_lines && index % major_every == 0) {
            continue;
        }
        let offset = index as f32 * step;
        push_asset_preview_grid_quad(&mut vertices, &mut indices, true, offset, half_width);
        push_asset_preview_grid_quad(&mut vertices, &mut indices, false, offset, half_width);
    }
    newengine_primitives::PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: PREVIEW_GRID_HALF_EXTENT * std::f32::consts::SQRT_2,
    }
}

fn asset_preview_axis_mesh(along_x: bool, half_width: f32) -> newengine_primitives::PrimitiveMesh {
    let mut vertices = Vec::with_capacity(4);
    let mut indices = Vec::with_capacity(6);
    push_asset_preview_grid_quad(&mut vertices, &mut indices, along_x, 0.0, half_width);
    newengine_primitives::PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: PREVIEW_GRID_HALF_EXTENT,
    }
}

fn draw_asset_preview_grid_layer(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    identity: &str,
    mesh: newengine_primitives::PrimitiveMesh,
    model: Mat4,
    color: [f32; 4],
) -> newengine_core::EngineResult<()> {
    let primitive_id =
        newengine_primitives::PrimitiveId::new(newengine_primitives::fnv1a_64(identity));
    let gpu = if let Some(gpu) = this.gpu.meshes.prim_cache.get(&primitive_id).copied() {
        gpu
    } else {
        let gpu = upload_primitive_mesh(r, &mesh, identity)?;
        this.gpu.meshes.prim_cache.insert(primitive_id, gpu);
        gpu
    };
    let material_plan = LitMaterialPlan::from_resolved(None, color);
    let pipeline = lit.double_sided_pipeline;
    let key = newengine_primitives::fnv1a_64(&format!(
        "asset.preview.grid.ubo:{identity}:{}",
        pipeline.get()
    ));
    let mut per = this.ensure_per_draw_ubo_with_binding(
        r,
        lit,
        key,
        lit.white_texture,
        lit.flat_normal_texture,
        lit.white_texture,
        lit.white_texture,
        lit.clamp_sampler,
    )?;
    per.last_seen_frame = this.frame.frame_index;
    this.gpu.material.per_draw_ubo.insert(key, per);
    crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
        r,
        per.ubo,
        viewproj * model,
        model,
        material_plan.base_color,
        [color[0] * 0.55, color[1] * 0.55, color[2] * 0.55],
        0.0,
        material_plan.uv_transform,
        material_plan.material_params,
        lights,
    )?;
    r.set_pipeline(pipeline)?;
    r.set_bind_group(0, per.bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
    r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
    this.diagnostics
        .overlay_metrics
        .record_indexed_triangles(gpu.index_count);
    Ok(())
}

fn draw_asset_preview_editor_grid(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    floor_y: f32,
) -> newengine_core::EngineResult<()> {
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.minor",
        asset_preview_grid_mesh(PREVIEW_GRID_MINOR_STEP, 0.004, true),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.018, 0.0)),
        [0.19, 0.22, 0.26, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.major",
        asset_preview_grid_mesh(PREVIEW_GRID_MAJOR_STEP, 0.009, false),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.012, 0.0)),
        [0.34, 0.38, 0.44, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.axis_x",
        asset_preview_axis_mesh(true, 0.014),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.006, 0.0)),
        [0.72, 0.20, 0.18, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.axis_z",
        asset_preview_axis_mesh(false, 0.014),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.006, 0.0)),
        [0.18, 0.42, 0.76, 1.0],
    )
}

pub fn draw_asset_preview_bundle(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    bundle: &newengine_model_domain_api::ModelAssetBundle,
    lit: newengine_material_domain_api::LitPipeline,
    viewport_extent: newengine_core::render::Extent2D,
    preview_view: newengine_render_feature_api::AssetPreviewView,
) -> newengine_core::EngineResult<()> {
    if bundle.parts.is_empty() {
        return Ok(());
    }

    // Use the exact vertex AABB. Per-part bounding spheres deliberately
    // overestimate the other axes and made tall characters float above the
    // editor grid while also shrinking them unnecessarily in the preview.
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut bounded_vertices = 0usize;
    for part in &bundle.parts {
        for vertex in &part.mesh.vertices {
            let position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            if !position.is_finite() {
                continue;
            }
            min[0] = min[0].min(position.x);
            min[1] = min[1].min(position.y);
            min[2] = min[2].min(position.z);
            max[0] = max[0].max(position.x);
            max[1] = max[1].max(position.y);
            max[2] = max[2].max(position.z);
            bounded_vertices += 1;
        }
    }
    if bounded_vertices == 0 {
        return Ok(());
    }
    let center = Vec3::new(
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    );
    let extent = Vec3::new(max[0] - min[0], max[1] - min[1], max[2] - min[2]);
    // Interaction orbits the camera. The model itself remains stable, so an
    // idle preview can keep its cached render target without per-frame work.
    let preview_scale = asset_preview_model_scale(extent);
    let model = asset_preview_model_transform(center, extent, 0.0);
    let floor_y = -extent.y * preview_scale * 0.5;

    let aspect = viewport_extent.width.max(1) as f32 / viewport_extent.height.max(1) as f32;
    let camera_target = Vec3::new(
        preview_view.target_offset[0],
        preview_view.target_offset[1],
        preview_view.target_offset[2],
    );
    let camera_position = camera_target + asset_preview_camera_position(preview_view);
    let view = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);
    // The authored UI samples a Vulkan render target as an image. Vulkan's
    // framebuffer Y direction is opposite to the mathematical camera space;
    // flip clip-space Y here so the preview texture is displayed upright.
    let mut projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.05, 100.0);
    projection.y_axis.y = -projection.y_axis.y;
    let viewproj = projection * view;
    let lights = PackedLights {
        ambient: [0.72, 0.76, 0.82, 0.9],
        dir_dir_intensity: [-0.45, -0.75, -0.5, 3.0],
        dir_color: [1.0, 0.97, 0.92, 0.0],
        ..PackedLights::default()
    }
    .with_camera_position([camera_position.x, camera_position.y, camera_position.z]);

    draw_asset_preview_editor_grid(this, r, lit, viewproj, &lights, floor_y)?;

    let force_double_sided = matches!(
        bundle.configuration.render_options.cull_policy,
        newengine_model_domain_api::MeshCullPolicy::None
    );
    let mut uploaded_parts = 0usize;
    for (index, part) in bundle.parts.iter().enumerate() {
        let identity = format!(
            "asset.preview:{}:{}:{}",
            bundle.source, bundle.dependency_graph.stable_cache_key, index
        );
        let primitive_id =
            newengine_primitives::PrimitiveId::new(newengine_primitives::fnv1a_64(&identity));
        let gpu = if let Some(gpu) = this.gpu.meshes.prim_cache.get(&primitive_id).copied() {
            gpu
        } else {
            let gpu = upload_primitive_mesh(r, &part.mesh, &identity)?;
            this.gpu.meshes.prim_cache.insert(primitive_id, gpu);
            uploaded_parts += 1;
            gpu
        };
        let resolved = newengine_materials::MaterialResolved {
            id: MaterialId::invalid(),
            desc: part.material.descriptor,
            textures: part.material.textures.clone(),
        };
        let material_plan =
            LitMaterialPlan::from_resolved(Some(&resolved), part.material.fallback_color);
        let base_texture = this.material_texture_or_default(
            r,
            material_plan.base_color_texture,
            lit.white_texture,
        );
        let normal_texture = this.material_texture_or_default(
            r,
            material_plan.normal_texture,
            lit.flat_normal_texture,
        );
        let roughness_texture =
            this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.alpha_cutoff > 0.0 {
            lit.clamp_sampler
        } else if material_plan.has_textures() {
            lit.repeat_sampler
        } else {
            lit.clamp_sampler
        };
        let pipeline = if force_double_sided || material_plan.double_sided {
            lit.double_sided_pipeline
        } else {
            lit.pipeline
        };
        let key = newengine_primitives::fnv1a_64(&format!(
            "asset.preview.ubo:{}:{}:{}:{}:{}",
            identity,
            pipeline.get(),
            base_texture.get(),
            normal_texture.get(),
            roughness_texture.get(),
        ));
        let mut per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            base_texture,
            normal_texture,
            roughness_texture,
            lit.white_texture,
            sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);
        crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            viewproj * model,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.alpha_cutoff,
            material_plan.uv_transform,
            material_plan.material_params,
            &lights,
        )?;
        r.set_pipeline(pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(gpu.index_count);
    }
    if uploaded_parts > 0 {
        newengine_ulog_api::ulog::info!(
            "asset preview: render packet uploaded source='{}' uploaded_parts={} total_parts={} graph_cache_key='{}' bounds_center=({:.3},{:.3},{:.3}) bounds_extent=({:.3},{:.3},{:.3}) preview_scale={:.6} first_base_color={:?}",
            bundle.source,
            uploaded_parts,
            bundle.parts.len(),
            bundle.dependency_graph.stable_cache_key,
            center.x,
            center.y,
            center.z,
            extent.x,
            extent.y,
            extent.z,
            2.2 / extent.x.max(extent.y).max(extent.z).max(0.001),
            bundle.parts.first().map(|part| part.material.descriptor.base_color)
        );
    }
    Ok(())
}

#[cfg(test)]
mod asset_preview_transform_tests {
    use super::*;

    #[test]
    fn preview_transform_keeps_off_origin_bounds_center_at_origin() {
        let center = Vec3::new(128.0, -32.0, 75.0);
        let extent = Vec3::new(10.0, 4.0, 7.0);
        let transform = asset_preview_model_transform(center, extent, 1.1);
        let transformed_center = transform.transform_point3(center);
        assert!(
            transformed_center.length() < 0.0001,
            "{transformed_center:?}"
        );
    }

    #[test]
    fn preview_camera_position_respects_requested_distance() {
        let view = newengine_render_feature_api::AssetPreviewView {
            yaw_radians: 1.1,
            pitch_radians: 0.4,
            distance: 5.25,
            target_offset: [0.0, 0.0, 0.0],
        };
        let position = asset_preview_camera_position(view);
        assert!((position.length() - 5.25).abs() < 0.0001);
        assert!(position.y > 0.0);
    }

    #[test]
    fn preview_transform_normalizes_largest_extent() {
        let transform = asset_preview_model_transform(Vec3::ZERO, Vec3::new(10.0, 4.0, 7.0), 0.0);
        let transformed = transform.transform_vector3(Vec3::new(10.0, 0.0, 0.0));
        assert!((transformed.length() - 2.2).abs() < 0.0001);
    }
}
