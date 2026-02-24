#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{BufferSlice, IndexFormat};
use newengine_math::{Mat4, Quat, Vec3};

use newengine_lighting::{DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_primitives::builtins as prim_builtins;
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use super::super::gpu::{ensure_grid, ensure_primitive_gpu, GridMeshParams};
use super::lights::PackedLights;
use super::EditorRenderController;

pub(super) fn publish_camera_spawn(
    bridge: &crate::viewport_bridge::ViewportBridge,
    rig: &newengine_camera::CameraRig,
) {
    let view = rig.view_matrix();
    let inv_view = view.inverse();
    let cam_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);
    let cam_fwd = -Vec3::new(inv_view.z_axis.x, inv_view.z_axis.y, inv_view.z_axis.z);
    bridge.publish_camera_spawn(cam_pos, cam_fwd);
}

pub(super) fn draw_grid(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    rig: &newengine_camera::CameraRig,
    focus: Vec3,
    bounds_radius: f32,
    lights: &PackedLights,
) -> newengine_core::EngineResult<()> {
    if !bounds_radius.is_finite() {
        return Ok(());
    }

    let g = ensure_grid(
        &mut this.grid,
        r,
        lit.bgl,
        GridMeshParams {
            half_lines: EditorRenderController::GRID_HALF_LINES,
            major_every: EditorRenderController::GRID_MAJOR_EVERY,
            minor_color: EditorRenderController::GRID_MINOR_COLOR,
            major_color: EditorRenderController::GRID_MAJOR_COLOR,
        },
    )?;

    let cam_dist = (rig.position - focus).length();
    let spacing = EditorRenderController::grid_spacing(cam_dist);

    let grid_model = Mat4::from_scale_rotation_translation(
        Vec3::new(spacing, 1.0, spacing),
        Quat::IDENTITY,
        Vec3::ZERO,
    );

    super::passes_ubo::write_lit_ubo(
        r,
        lit.grid_ubo,
        viewproj * grid_model,
        grid_model,
        [1.0, 1.0, 1.0, 1.0],
        lights,
    )?;

    r.set_pipeline(g.pipeline)?;
    r.set_bind_group(0, lit.grid_bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
    r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
    Ok(())
}

pub(super) fn draw_primitives(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.prim_cache, r)?;

        let model = gt.0;
        let mvp = viewproj * model;

        let base_color = world
            .get::<newengine_materials::MaterialRef>(id)
            .and_then(|mr| mats.get(mr.id))
            .map(|d| d.base_color)
            .unwrap_or([1.0, 0.0, 1.0, 1.0]);

        let key = id.stable_u64();
        let mut per = this.ensure_per_draw_ubo(r, lit, key)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo(r, per.ubo, mvp, model, base_color, lights)?;

        r.set_pipeline(lit.pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(
            gpu.index_count,
        ))?;
    }

    Ok(())
}

pub(super) fn draw_light_gizmos(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    quat_from_forward_z: fn(Vec3) -> Quat,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();

    let sphere_id = prim_builtins::ID_SPHERE_UV;
    let sphere_gpu = ensure_primitive_gpu(&reg, sphere_id, &mut this.prim_cache, r)?;

    let cone_id = prim_builtins::ID_CONE;
    let cone_gpu = ensure_primitive_gpu(&reg, cone_id, &mut this.prim_cache, r)?;

    let mut dirs: Vec<(u64, DirectionalLight, Mat4)> = Vec::new();
    for (e, l, gt) in world.query2::<DirectionalLight, GlobalTransform>() {
        dirs.push((e.stable_u64(), *l, gt.0));
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    for (k, dl, m) in dirs {
        let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
        let dir = Vec3::new(dl.direction_ws[0], dl.direction_ws[1], dl.direction_ws[2])
            .normalize_or_zero();

        let rot = quat_from_forward_z(dir);
        let scale = Vec3::splat(0.35);
        let model = Mat4::from_scale_rotation_translation(scale, rot, pos);
        let mvp = viewproj * model;

        let base_color = [1.0, 0.95, 0.35, 1.0];
        let mut per = this.ensure_per_draw_ubo(r, lit, k)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(k, per);
        super::passes_ubo::write_lit_ubo(r, per.ubo, mvp, model, base_color, lights)?;

        r.set_pipeline(lit.pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(cone_gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(cone_gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(
            cone_gpu.index_count,
        ))?;

        let line_len = 1.2_f32;
        let line_pos = pos + dir * (line_len * 0.5);
        let line_scale = Vec3::new(0.08, 0.08, line_len);
        let line_model = Mat4::from_scale_rotation_translation(line_scale, rot, line_pos);
        let line_mvp = viewproj * line_model;
        let line_color = [1.0, 0.85, 0.25, 1.0];
        let line_key = k ^ 0xD1A1_0000_0000_0000u64;
        let mut per2 = this.ensure_per_draw_ubo(r, lit, line_key)?;
        per2.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(line_key, per2);
        super::passes_ubo::write_lit_ubo(r, per2.ubo, line_mvp, line_model, line_color, lights)?;

        r.set_pipeline(lit.pipeline)?;
        r.set_bind_group(0, per2.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(cone_gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(cone_gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(
            cone_gpu.index_count,
        ))?;
    }

    let mut pts: Vec<(u64, Mat4)> = Vec::new();
    for (e, _pl, gt) in world.query2::<PointLight, GlobalTransform>() {
        pts.push((e.stable_u64(), gt.0));
    }
    pts.sort_by(|a, b| a.0.cmp(&b.0));

    for (k, m) in pts {
        let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
        let scale = Vec3::splat(0.18);
        let model = Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, pos);
        let mvp = viewproj * model;

        let base_color = [1.0, 0.75, 0.25, 1.0];
        let mut per = this.ensure_per_draw_ubo(r, lit, k)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(k, per);
        super::passes_ubo::write_lit_ubo(r, per.ubo, mvp, model, base_color, lights)?;

        r.set_pipeline(lit.pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(sphere_gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(sphere_gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(
            sphere_gpu.index_count,
        ))?;
    }

    Ok(())
}
