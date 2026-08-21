use super::*;

pub(super) fn draw_editor_viewport_overlays(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
) -> newengine_core::EngineResult<()> {
    use newengine_core::render::{BufferSlice, DrawArgs};
    use newengine_math::Vec4;

    if !this.editor_viewport.is_active() {
        return Ok(());
    }
    let state = this.editor_viewport.state();
    if !state.show_grid && !state.show_bounds && !state.show_collision {
        return Ok(());
    }

    let world = scene.world();
    let mut bytes = Vec::<u8>::new();
    let mut vertex_count = 0usize;
    let mut push_line = |a: Vec3, b: Vec3, color: [f32; 4]| {
        for position in [a, b] {
            let clip = viewproj * Vec4::new(position.x, position.y, position.z, 1.0);
            for value in [
                clip.x, clip.y, clip.z, clip.w, color[0], color[1], color[2], color[3],
            ] {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
            vertex_count += 1;
        }
    };

    if state.show_grid {
        let step = state.translation_snap_units.max(1.0);
        let half_cells = 20i32;
        let extent = step * half_cells as f32;
        let minor = [0.24, 0.26, 0.29, 0.75];
        let major = [0.38, 0.41, 0.45, 0.92];
        for cell in -half_cells..=half_cells {
            let offset = cell as f32 * step;
            let color = if cell == 0 || cell % 5 == 0 {
                major
            } else {
                minor
            };
            push_line(
                Vec3::new(-extent, 0.0, offset),
                Vec3::new(extent, 0.0, offset),
                color,
            );
            push_line(
                Vec3::new(offset, 0.0, -extent),
                Vec3::new(offset, 0.0, extent),
                color,
            );
        }
    }

    if state.show_bounds {
        if let Some(selected) = this.bridges.scene.selection() {
            if let (Some(bounds), Some(global)) = (
                world.get::<Bounds>(selected),
                world.get::<GlobalTransform>(selected),
            ) {
                let (center, radius) = transform_sphere(
                    global.0,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                push_wire_cube(
                    &mut push_line,
                    center,
                    Vec3::splat(radius),
                    [1.0, 0.72, 0.12, 1.0],
                );
            }
        }
    }

    if state.show_collision {
        for (entity, body, global) in
            world.query2::<crate::gameplay::PhysicsBodyDesc, GlobalTransform>()
        {
            if world
                .get::<crate::editor_viewport::EditorGizmoAxisComponent>(entity)
                .is_some()
            {
                continue;
            }
            let color = if body.is_trigger() {
                [0.85, 0.25, 0.86, 1.0]
            } else {
                [0.20, 0.92, 0.38, 1.0]
            };
            match body.shape.sanitized() {
                newengine_physics_contracts::CollisionShapeDesc::Box { half_extents } => {
                    push_wire_box_transform(
                        &mut push_line,
                        global.0,
                        Vec3::new(half_extents[0], half_extents[1], half_extents[2]),
                        color,
                    );
                }
                newengine_physics_contracts::CollisionShapeDesc::Sphere { radius } => {
                    push_wire_sphere_transform(&mut push_line, global.0, radius, color);
                }
                newengine_physics_contracts::CollisionShapeDesc::Capsule {
                    radius,
                    half_height,
                } => {
                    push_wire_capsule_transform(
                        &mut push_line,
                        global.0,
                        radius,
                        half_height,
                        color,
                    );
                }
            }
        }
    }

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

fn push_wire_box_transform(
    push_line: &mut impl FnMut(Vec3, Vec3, [f32; 4]),
    transform: Mat4,
    half_extents: Vec3,
    color: [f32; 4],
) {
    let h = half_extents;
    let local = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    let p = local.map(|point| transform.transform_point3(point));
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        push_line(p[a], p[b], color);
    }
}

fn push_wire_sphere_transform(
    push_line: &mut impl FnMut(Vec3, Vec3, [f32; 4]),
    transform: Mat4,
    radius: f32,
    color: [f32; 4],
) {
    const SEGMENTS: usize = 24;
    for plane in 0..3 {
        let mut previous = None;
        for segment in 0..=SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * core::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let local = match plane {
                0 => Vec3::new(0.0, c * radius, s * radius),
                1 => Vec3::new(c * radius, 0.0, s * radius),
                _ => Vec3::new(c * radius, s * radius, 0.0),
            };
            let world = transform.transform_point3(local);
            if let Some(prev) = previous {
                push_line(prev, world, color);
            }
            previous = Some(world);
        }
    }
}

fn push_wire_capsule_transform(
    push_line: &mut impl FnMut(Vec3, Vec3, [f32; 4]),
    transform: Mat4,
    radius: f32,
    half_height: f32,
    color: [f32; 4],
) {
    const SEGMENTS: usize = 20;
    let top = half_height;
    let bottom = -half_height;
    for y in [bottom, top] {
        let mut previous = None;
        for segment in 0..=SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * core::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let world = transform.transform_point3(Vec3::new(c * radius, y, s * radius));
            if let Some(prev) = previous {
                push_line(prev, world, color);
            }
            previous = Some(world);
        }
    }
    for (x, z) in [(radius, 0.0), (-radius, 0.0), (0.0, radius), (0.0, -radius)] {
        push_line(
            transform.transform_point3(Vec3::new(x, bottom, z)),
            transform.transform_point3(Vec3::new(x, top, z)),
            color,
        );
    }
    for plane in 0..2 {
        for upper in [false, true] {
            let center_y = if upper { top } else { bottom };
            let start = if upper { 0.0 } else { core::f32::consts::PI };
            let mut previous = None;
            for segment in 0..=SEGMENTS / 2 {
                let t = segment as f32 / (SEGMENTS / 2) as f32;
                let angle = start + t * core::f32::consts::PI;
                let (s, c) = angle.sin_cos();
                let local = if plane == 0 {
                    Vec3::new(c * radius, center_y + s * radius, 0.0)
                } else {
                    Vec3::new(0.0, center_y + s * radius, c * radius)
                };
                let world = transform.transform_point3(local);
                if let Some(prev) = previous {
                    push_line(prev, world, color);
                }
                previous = Some(world);
            }
        }
    }
}

fn push_wire_cube(
    push_line: &mut impl FnMut(Vec3, Vec3, [f32; 4]),
    center: Vec3,
    half_extents: Vec3,
    color: [f32; 4],
) {
    let h = half_extents;
    let p = [
        center + Vec3::new(-h.x, -h.y, -h.z),
        center + Vec3::new(h.x, -h.y, -h.z),
        center + Vec3::new(h.x, h.y, -h.z),
        center + Vec3::new(-h.x, h.y, -h.z),
        center + Vec3::new(-h.x, -h.y, h.z),
        center + Vec3::new(h.x, -h.y, h.z),
        center + Vec3::new(h.x, h.y, h.z),
        center + Vec3::new(-h.x, h.y, h.z),
    ];
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        push_line(p[a], p[b], color);
    }
}
