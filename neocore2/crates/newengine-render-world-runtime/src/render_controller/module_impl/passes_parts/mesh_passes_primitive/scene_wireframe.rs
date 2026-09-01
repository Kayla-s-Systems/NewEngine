use super::*;

pub(super) fn draw_primitives_wireframe(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    use newengine_core::render::{BufferSlice, DrawArgs};

    const MAX_WIREFRAME_VERTICES: usize = 240_000;
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mut bytes = Vec::<u8>::new();
    let mut vertex_count = 0usize;

    'primitive_entities: for (entity, primitive, global) in
        world.query2::<Primitive, GlobalTransform>()
    {
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let Ok(mesh) = reg.build_mesh(primitive.id) else {
            continue;
        };
        if !append_wire_mesh_edges(
            &mesh,
            global.0,
            primitive.color,
            viewproj,
            MAX_WIREFRAME_VERTICES,
            &mut bytes,
            &mut vertex_count,
        ) {
            break 'primitive_entities;
        }
    }
    drop(reg);

    if vertex_count < 2 {
        return Ok(());
    }
    let gpu = crate::render_controller::gpu::ensure_debug_line_pipeline(
        &mut this.gpu.meshes.collision_lines,
        r,
        vertex_count as u32,
    )?;
    r.write_buffer(gpu.vb, 0, &bytes)?;
    r.set_pipeline(gpu.pipeline)?;
    r.set_bind_group(0, gpu.bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.draw(DrawArgs::new(vertex_count as u32))?;
    Ok(())
}

fn append_wire_mesh_edges(
    mesh: &newengine_primitives::PrimitiveMesh,
    model: Mat4,
    color: [f32; 4],
    viewproj: Mat4,
    vertex_budget: usize,
    bytes: &mut Vec<u8>,
    vertex_count: &mut usize,
) -> bool {
    use newengine_math::Vec4;

    for triangle in mesh.indices.as_chunks::<3>().0 {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            if *vertex_count + 2 > vertex_budget {
                return false;
            }
            let Some(a) = mesh.vertices.get(a as usize) else {
                continue;
            };
            let Some(b) = mesh.vertices.get(b as usize) else {
                continue;
            };
            for vertex in [a, b] {
                let position =
                    model.transform_point3(Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]));
                let clip = viewproj * Vec4::new(position.x, position.y, position.z, 1.0);
                for value in [
                    clip.x, clip.y, clip.z, clip.w, color[0], color[1], color[2], color[3],
                ] {
                    bytes.extend_from_slice(&value.to_ne_bytes());
                }
                *vertex_count += 1;
            }
        }
    }
    true
}
