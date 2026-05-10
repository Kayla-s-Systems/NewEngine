#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{BufferSlice, DrawArgs, IndexFormat, TextureId};
use newengine_math::{Mat4, Quat, Vec3, Vec4};

use newengine_lighting::{DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_primitives::builtins as prim_builtins;
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use super::super::gpu::{
    ensure_debug_line_pipeline, ensure_grid, ensure_primitive_gpu, upload_primitive_mesh, GridMeshParams,
};
use super::super::material_bindings::LitMaterialPlan;
use super::grid;
use super::lights::PackedLights;
use super::EditorRenderController;
use crate::gameplay::{display_visible_in_mode, CollisionBody, CollisionShape};
use newengine_procedural_noise::ProceduralTerrain;

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
            half_lines: grid::HALF_LINES,
            major_every: grid::MAJOR_EVERY,
            minor_color: grid::MINOR_COLOR,
            major_color: grid::MAJOR_COLOR,
        },
    )?;

    let grid_model = grid::model_from_camera(rig);

    super::passes_ubo::write_lit_ubo(
        r,
        lit.grid_ubo,
        viewproj * grid_model,
        grid_model,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        lights,
    )?;

    r.set_pipeline(g.pipeline)?;
    r.set_bind_group(0, lit.grid_bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
    r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
    Ok(())
}

pub(super) fn draw_procedural_terrain(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(u64, ProceduralTerrain, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        entries.push((
            id.stable_u64(),
            terrain.clone(),
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (entity_key, terrain, model, material) in entries {
        let mesh_key = terrain.mesh_key();
        let gpu = if let Some(gpu) = this.terrain_cache.get(&mesh_key).copied() {
            gpu
        } else {
            let mesh = terrain.heightfield.to_primitive_mesh();
            let gpu = upload_primitive_mesh(r, &mesh, "editor_proc_terrain")?;
            this.terrain_cache.insert(mesh_key, gpu);
            gpu
        };

        let mvp = viewproj * model;
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), terrain.base_color);
        let base_tex = this.material_texture_or_default(r, material_plan.base_color_texture, lit.white_texture);
        let normal_tex = this.material_texture_or_default(r, material_plan.normal_texture, lit.flat_normal_texture);
        let roughness_tex = this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.has_textures() { lit.repeat_sampler } else { lit.clamp_sampler };

        let key = entity_key ^ 0x7e44_1000_0000_0000u64;
        let mut per = this.ensure_per_draw_ubo_with_binding(r, lit, key, base_tex, normal_tex, roughness_tex, shadow_texture, sampler)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        r.set_pipeline(if material_plan.double_sided { lit.double_sided_pipeline } else { lit.pipeline })?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(gpu.index_count))?;
    }

    Ok(())
}

pub(super) fn draw_primitives(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.prim_cache, r)?;

        let model = gt.0;
        let mvp = viewproj * model;

        let resolved = world
            .get::<newengine_materials::MaterialRef>(id)
            .and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 0.0, 1.0, 1.0]);
        let base_tex = this.material_texture_or_default(r, material_plan.base_color_texture, lit.white_texture);
        let normal_tex = this.material_texture_or_default(r, material_plan.normal_texture, lit.flat_normal_texture);
        let roughness_tex = this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.has_textures() { lit.repeat_sampler } else { lit.clamp_sampler };

        let key = id.stable_u64();
        let mut per = this.ensure_per_draw_ubo_with_binding(r, lit, key, base_tex, normal_tex, roughness_tex, shadow_texture, sampler)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        r.set_pipeline(if material_plan.double_sided { lit.double_sided_pipeline } else { lit.pipeline })?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(
            gpu.index_count,
        ))?;
    }

    Ok(())
}


pub(super) fn draw_procedural_terrain_shadow(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(u64, ProceduralTerrain, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        entries.push((
            id.stable_u64(),
            terrain.clone(),
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (entity_key, terrain, model, material) in entries {
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), terrain.base_color);
        if !material_plan.cast_shadows {
            continue;
        }

        let mesh_key = terrain.mesh_key();
        let gpu = if let Some(gpu) = this.terrain_cache.get(&mesh_key).copied() {
            gpu
        } else {
            let mesh = terrain.heightfield.to_primitive_mesh();
            let gpu = upload_primitive_mesh(r, &mesh, "editor_proc_terrain")?;
            this.terrain_cache.insert(mesh_key, gpu);
            gpu
        };

        let key = entity_key ^ 0x5a44_1000_0000_0000u64;
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
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        let mvp = light_viewproj * model;
        super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        r.set_pipeline(if material_plan.double_sided { lit.shadow_double_sided_pipeline } else { lit.shadow_pipeline })?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(gpu.index_count))?;
    }

    Ok(())
}

pub(super) fn draw_primitives_shadow(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }

        let resolved = world
            .get::<newengine_materials::MaterialRef>(id)
            .and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 1.0, 1.0, 1.0]);
        if !material_plan.cast_shadows {
            continue;
        }

        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.prim_cache, r)?;
        let model = gt.0;
        let mvp = light_viewproj * model;

        let key = id.stable_u64() ^ 0x5a50_0000_0000_0000u64;
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
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        r.set_pipeline(if material_plan.double_sided { lit.shadow_double_sided_pipeline } else { lit.shadow_pipeline })?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(newengine_core::render::DrawIndexedArgs::new(gpu.index_count))?;
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
    runtime: bool,
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
        if !display_visible_in_mode(world, e, runtime) {
            continue;
        }
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
        super::passes_ubo::write_lit_ubo(r, per.ubo, mvp, model, base_color, [0.0, 0.0, 0.0], lights)?;

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
        super::passes_ubo::write_lit_ubo(r, per2.ubo, line_mvp, line_model, line_color, [0.0, 0.0, 0.0], lights)?;

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
        if !display_visible_in_mode(world, e, runtime) {
            continue;
        }
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
        super::passes_ubo::write_lit_ubo(r, per.ubo, mvp, model, base_color, [0.0, 0.0, 0.0], lights)?;

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


#[inline]
fn push_clip_vertex(bytes: &mut Vec<u8>, clip: Vec4, color: [f32; 4]) {
    for v in [clip.x, clip.y, clip.z, clip.w, color[0], color[1], color[2], color[3]] {
        bytes.extend_from_slice(&v.to_ne_bytes());
    }
}

#[inline]
fn push_segment(bytes: &mut Vec<u8>, viewproj: Mat4, a: Vec3, b: Vec3, color: [f32; 4]) {
    let ca = viewproj * Vec4::new(a.x, a.y, a.z, 1.0);
    let cb = viewproj * Vec4::new(b.x, b.y, b.z, 1.0);
    push_clip_vertex(bytes, ca, color);
    push_clip_vertex(bytes, cb, color);
}

#[inline]
fn push_box_wire(bytes: &mut Vec<u8>, viewproj: Mat4, model: Mat4, half_extents: [f32; 3], color: [f32; 4]) {
    let hx = half_extents[0].max(0.001);
    let hy = half_extents[1].max(0.001);
    let hz = half_extents[2].max(0.001);
    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];
    let edges = [
        (0usize, 1usize), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    for (a, b) in edges {
        push_segment(bytes, viewproj, model.transform_point3(corners[a]), model.transform_point3(corners[b]), color);
    }
}

#[inline]
fn push_circle_wire(
    bytes: &mut Vec<u8>,
    viewproj: Mat4,
    model: Mat4,
    color: [f32; 4],
    segments: usize,
    f: impl Fn(f32) -> Vec3,
) {
    let segs = segments.max(8);
    for i in 0..segs {
        let t0 = (i as f32 / segs as f32) * core::f32::consts::TAU;
        let t1 = ((i + 1) as f32 / segs as f32) * core::f32::consts::TAU;
        push_segment(
            bytes,
            viewproj,
            model.transform_point3(f(t0)),
            model.transform_point3(f(t1)),
            color,
        );
    }
}

#[inline]
fn push_sphere_wire(bytes: &mut Vec<u8>, viewproj: Mat4, model: Mat4, radius: f32, color: [f32; 4]) {
    let r = radius.max(0.001);
    push_circle_wire(bytes, viewproj, model, color, 24, |t| Vec3::new(t.cos() * r, 0.0, t.sin() * r));
    push_circle_wire(bytes, viewproj, model, color, 24, |t| Vec3::new(t.cos() * r, t.sin() * r, 0.0));
    push_circle_wire(bytes, viewproj, model, color, 24, |t| Vec3::new(0.0, t.cos() * r, t.sin() * r));
}

#[inline]
fn push_capsule_wire(
    bytes: &mut Vec<u8>,
    viewproj: Mat4,
    model: Mat4,
    radius: f32,
    half_height: f32,
    color: [f32; 4],
) {
    let r = radius.max(0.001);
    let hh = half_height.max(0.0);
    let top = hh;
    let bottom = -hh;

    push_circle_wire(bytes, viewproj, model, color, 24, |t| Vec3::new(t.cos() * r, top, t.sin() * r));
    push_circle_wire(bytes, viewproj, model, color, 24, |t| Vec3::new(t.cos() * r, bottom, t.sin() * r));

    for sx in [-1.0_f32, 1.0_f32] {
        for sz in [-1.0_f32, 1.0_f32] {
            push_segment(
                bytes,
                viewproj,
                model.transform_point3(Vec3::new(sx * r, top, sz * 0.0)),
                model.transform_point3(Vec3::new(sx * r, bottom, sz * 0.0)),
                color,
            );
            push_segment(
                bytes,
                viewproj,
                model.transform_point3(Vec3::new(sz * 0.0, top, sx * r)),
                model.transform_point3(Vec3::new(sz * 0.0, bottom, sx * r)),
                color,
            );
        }
    }

    push_circle_wire(bytes, viewproj, model, color, 16, |t| {
        Vec3::new(t.cos() * r, top + t.sin().max(0.0) * r, 0.0)
    });
    push_circle_wire(bytes, viewproj, model, color, 16, |t| {
        Vec3::new(0.0, top + t.sin().max(0.0) * r, t.cos() * r)
    });
    push_circle_wire(bytes, viewproj, model, color, 16, |t| {
        Vec3::new(t.cos() * r, bottom + t.sin().min(0.0) * r, 0.0)
    });
    push_circle_wire(bytes, viewproj, model, color, 16, |t| {
        Vec3::new(0.0, bottom + t.sin().min(0.0) * r, t.cos() * r)
    });
}

pub(super) fn draw_collision_wireframe(
    this: &mut EditorRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
) -> newengine_core::EngineResult<()> {
    if !this.scene_bridge.collision_wireframe_enabled() {
        return Ok(());
    }

    let world = scene.world();
    let selected = this.scene_bridge.selection();
    let mut entries: Vec<(u64, CollisionBody, Mat4)> = world
        .query2::<CollisionBody, GlobalTransform>()
        .map(|(e, body, gt)| (e.stable_u64(), *body, gt.0))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    if entries.is_empty() {
        return Ok(());
    }

    let mut bytes: Vec<u8> = Vec::with_capacity(entries.len() * 32 * 32);
    for (id, body, model) in entries {
        let is_selected = selected.map(|e| e.stable_u64()) == Some(id);
        let color = if is_selected {
            [0.20, 0.95, 0.95, 1.0]
        } else if body.is_trigger {
            [0.95, 0.65, 0.20, 1.0]
        } else if body.dynamic {
            [0.35, 0.85, 0.35, 1.0]
        } else {
            [0.35, 0.65, 1.0, 1.0]
        };

        match body.shape {
            CollisionShape::Box { half_extents } => {
                push_box_wire(&mut bytes, viewproj, model, half_extents, color);
            }
            CollisionShape::Sphere { radius } => {
                push_sphere_wire(&mut bytes, viewproj, model, radius, color);
            }
            CollisionShape::Capsule { radius, half_height } => {
                push_capsule_wire(&mut bytes, viewproj, model, radius, half_height, color);
            }
        }
    }

    let vertex_count = (bytes.len() / 32) as u32;
    if vertex_count == 0 {
        return Ok(());
    }

    let gpu = ensure_debug_line_pipeline(&mut this.collision_lines, r, vertex_count)?;
    r.write_buffer(gpu.vb, 0, &bytes)?;
    r.set_pipeline(gpu.pipeline)?;
    r.set_bind_group(0, gpu.bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.draw(DrawArgs::new(vertex_count))?;
    Ok(())
}
