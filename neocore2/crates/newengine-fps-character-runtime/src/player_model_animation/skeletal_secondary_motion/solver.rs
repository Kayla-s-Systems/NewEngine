fn project_out_of_secondary_motion_capsule(point: &mut Vec3, a: Vec3, b: Vec3, radius: f32) {
    let axis = b - a;
    let len2 = axis.length_squared();
    if len2 <= 1.0e-8 {
        return;
    }
    let t = ((*point - a).dot(axis) / len2).clamp(0.0, 1.0);
    let closest = a + axis * t;
    let delta = *point - closest;
    let distance = delta.length();
    if distance < radius {
        let normal = if distance > 1.0e-6 {
            delta / distance
        } else {
            Vec3::Z
        };
        *point = closest + normal * radius;
    }
}

fn project_behind_secondary_motion_capsule(
    point: &mut Vec3,
    capsule: SecondaryMotionCapsule,
    back_normal: Vec3,
    margin: f32,
) {
    let axis = capsule.b - capsule.a;
    let len2 = axis.length_squared();
    if len2 <= 1.0e-8 {
        return;
    }
    let t = ((*point - capsule.a).dot(axis) / len2).clamp(0.0, 1.0);
    let closest = capsule.a + axis * t;
    let back = back_normal.normalize_or_zero();
    if back.length_squared() <= 1.0e-8 {
        return;
    }
    let delta = *point - closest;
    let signed = delta.dot(back);
    let tangent = (delta - back * signed).length();
    let radius = capsule.radius + margin;
    if tangent < radius * 1.10 && signed < radius {
        *point += back * (radius - signed);
    }
}

fn project_behind_secondary_motion_box(
    point: &mut Vec3,
    box_shape: SecondaryMotionOrientedBox,
    tunnel_depth: f32,
) {
    let delta = *point - box_shape.center;
    let local = Vec3::new(
        delta.dot(box_shape.axes[0]),
        delta.dot(box_shape.axes[1]),
        delta.dot(box_shape.axes[2]),
    );
    let extents = box_shape.half_extents;
    if local.x.abs() > extents.x || local.y.abs() > extents.y {
        return;
    }
    if local.z < extents.z && local.z > -(extents.z + tunnel_depth.max(0.0)) {
        *point += box_shape.axes[2] * (extents.z - local.z);
    }
}

fn project_out_of_secondary_motion_box(point: &mut Vec3, box_shape: SecondaryMotionOrientedBox) {
    let delta = *point - box_shape.center;
    let local = Vec3::new(
        delta.dot(box_shape.axes[0]),
        delta.dot(box_shape.axes[1]),
        delta.dot(box_shape.axes[2]),
    );
    let extents = box_shape.half_extents;
    if local.x.abs() >= extents.x || local.y.abs() >= extents.y || local.z.abs() >= extents.z {
        return;
    }
    let distances = [
        extents.x - local.x.abs(),
        extents.y - local.y.abs(),
        extents.z - local.z.abs(),
    ];
    let axis = if distances[0] <= distances[1] && distances[0] <= distances[2] {
        0
    } else if distances[1] <= distances[2] {
        1
    } else {
        2
    };
    let component = match axis {
        0 => local.x,
        1 => local.y,
        _ => local.z,
    };
    let sign = if component >= 0.0 { 1.0 } else { -1.0 };
    *point += box_shape.axes[axis] * distances[axis] * sign;
}

fn pin_secondary_motion_particles(
    points: &mut [Vec3],
    guide: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
) {
    for (index, particle) in authored.particles.iter().enumerate() {
        if particle.mobility <= 1.0e-8 {
            points[index] = guide[index];
        }
    }
}

fn solve_secondary_motion_edge(
    points: &mut [Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    a: usize,
    b: usize,
    rest: f32,
    authored_stiffness: f32,
) {
    let delta = points[b] - points[a];
    let length = delta.length();
    if length <= 1.0e-6 || !length.is_finite() {
        return;
    }
    let wa = authored.particles[a].mobility.max(0.0);
    let wb = authored.particles[b].mobility.max(0.0);
    let weight_sum = wa + wb;
    if weight_sum <= 1.0e-8 {
        return;
    }
    let stiffness = (authored_stiffness / authored.tuning.stretch_reference_stiffness.max(1.0e-6))
        .clamp(0.0, 1.0);
    let correction = delta * (((length - rest) / length) * stiffness);
    points[a] += correction * (wa / weight_sum);
    points[b] -= correction * (wb / weight_sum);
}

fn damp_secondary_motion_edge_velocity(
    points: &[Vec3],
    previous: &mut [Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    a: usize,
    b: usize,
    authored_damping: f32,
) {
    let axis = (points[b] - points[a]).normalize_or_zero();
    if axis.length_squared() <= 1.0e-8 {
        return;
    }
    let wa = authored.particles[a].mobility.max(0.0);
    let wb = authored.particles[b].mobility.max(0.0);
    let weight_sum = wa + wb;
    if weight_sum <= 1.0e-8 {
        return;
    }
    let mut va = points[a] - previous[a];
    let mut vb = points[b] - previous[b];
    let relative = (vb - va).dot(axis);
    let damping = authored_damping.clamp(0.0, 1.0);
    va += axis * (relative * damping * wa / weight_sum);
    vb -= axis * (relative * damping * wb / weight_sum);
    previous[a] = points[a] - va;
    previous[b] = points[b] - vb;
}

fn solve_secondary_motion_bend(
    points: &mut [Vec3],
    guide: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    indices: [usize; 4],
    weights: [f32; 4],
    geometry_scale: f32,
    rest_scalar: f32,
) {
    let mut current = Vec3::ZERO;
    let mut target = Vec3::ZERO;
    let mut denominator = 0.0f32;
    for lane in 0..4 {
        let index = indices[lane];
        let weight = weights[lane];
        current += points[index] * weight;
        target += guide[index] * weight;
        denominator += authored.particles[index].mobility.max(0.0) * weight * weight;
    }
    if denominator <= 1.0e-8 {
        return;
    }

    let bend_reference = authored.tuning.bend_reference_stiffness;
    let geometry_normalization =
        (bend_reference / geometry_scale.max(bend_reference)).clamp(0.0, 1.0);
    let rest_modulation = (1.0 + rest_scalar.abs() / 0.001).recip();
    let stiffness = (geometry_normalization * rest_modulation).clamp(0.0, 1.0);
    let error = (current - target) * stiffness;
    for lane in 0..4 {
        let index = indices[lane];
        let mobility = authored.particles[index].mobility.max(0.0);
        if mobility <= 1.0e-8 {
            continue;
        }
        points[index] -= error * (mobility * weights[lane] / denominator);
    }
}

fn secondary_motion_centerline_into(
    points: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    out: &mut Vec<Vec3>,
) {
    out.clear();
    if out.capacity() < authored.centerline_pairs.len() {
        out.reserve(authored.centerline_pairs.len() - out.capacity());
    }
    out.extend(
        authored
            .centerline_pairs
            .iter()
            .map(|pair| (points[pair[0]] + points[pair[1]]) * 0.5),
    );
}

fn normalized_polyline_parameter(points: &[Vec3], index: usize) -> f32 {
    if points.len() <= 1 || index == 0 {
        return 0.0;
    }
    let mut total = 0.0f32;
    let mut prefix = 0.0f32;
    for segment in 0..points.len() - 1 {
        let length = (points[segment + 1] - points[segment]).length();
        total += length;
        if segment < index {
            prefix += length;
        }
    }
    if total <= 1.0e-8 {
        0.0
    } else {
        (prefix / total).clamp(0.0, 1.0)
    }
}

fn sample_polyline_normalized(points: &[Vec3], t: f32) -> Vec3 {
    if points.is_empty() {
        return Vec3::ZERO;
    }
    if points.len() == 1 {
        return points[0];
    }
    let mut total = 0.0f32;
    for segment in 0..points.len() - 1 {
        total += (points[segment + 1] - points[segment]).length();
    }
    if total <= 1.0e-8 {
        return points[0];
    }
    let target = t.clamp(0.0, 1.0) * total;
    let mut cursor = 0.0f32;
    for segment in 0..points.len() - 1 {
        let length = (points[segment + 1] - points[segment]).length();
        if target <= cursor + length || segment + 2 == points.len() {
            let local = if length <= 1.0e-8 {
                0.0
            } else {
                ((target - cursor) / length).clamp(0.0, 1.0)
            };
            return points[segment].lerp(points[segment + 1], local);
        }
        cursor += length;
    }
    *points.last().unwrap_or(&points[0])
}

