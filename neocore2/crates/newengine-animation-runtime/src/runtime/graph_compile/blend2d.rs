use super::*;

const MAX_BLEND2D_SAMPLES: usize = 128;
const BLEND2D_POINT_EPSILON: f32 = 1.0e-5;

pub(super) fn blend2d_position_distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn validate_blend2d_positions(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<(), String> {
    if samples.len() < 2 {
        return Err(format!(
            "animation graph '{graph_name}' blend2d requires at least two samples"
        ));
    }
    if samples.len() > MAX_BLEND2D_SAMPLES {
        return Err(format!(
            "animation graph '{graph_name}' blend2d exceeds sample budget samples={} limit={MAX_BLEND2D_SAMPLES}",
            samples.len()
        ));
    }
    let epsilon2 = BLEND2D_POINT_EPSILON * BLEND2D_POINT_EPSILON;
    for left in 0..samples.len() {
        for right in left + 1..samples.len() {
            if blend2d_position_distance_squared(samples[left].position, samples[right].position)
                <= epsilon2
            {
                return Err(format!(
                    "animation graph '{graph_name}' blend2d contains duplicate sample position [{}, {}]",
                    samples[right].position[0], samples[right].position[1]
                ));
            }
        }
    }
    Ok(())
}

fn compile_cartesian_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    let mut x_values = samples
        .iter()
        .map(|sample| sample.position[0])
        .collect::<Vec<_>>();
    let mut y_values = samples
        .iter()
        .map(|sample| sample.position[1])
        .collect::<Vec<_>>();
    x_values.sort_by(f32::total_cmp);
    y_values.sort_by(f32::total_cmp);
    x_values.dedup_by(|a, b| *a == *b);
    y_values.dedup_by(|a, b| *a == *b);
    if x_values.len() < 2 || y_values.len() < 2 {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d requires at least two unique values on each axis"
        ));
    }
    let expected = x_values
        .len()
        .checked_mul(y_values.len())
        .ok_or_else(|| format!("animation graph '{graph_name}' cartesian blend2d grid overflow"))?;
    if expected != samples.len() {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d requires a complete rectangular lattice samples={} expected={} x={} y={}",
            samples.len(), expected, x_values.len(), y_values.len()
        ));
    }
    let mut grid = vec![usize::MAX; expected];
    for (sample_index, sample) in samples.iter().enumerate() {
        let x = x_values
            .binary_search_by(|value| value.total_cmp(&sample.position[0]))
            .map_err(|_| {
                format!("animation graph '{graph_name}' cartesian blend2d x lookup failed")
            })?;
        let y = y_values
            .binary_search_by(|value| value.total_cmp(&sample.position[1]))
            .map_err(|_| {
                format!("animation graph '{graph_name}' cartesian blend2d y lookup failed")
            })?;
        let cell = y * x_values.len() + x;
        if grid[cell] != usize::MAX {
            return Err(format!(
                "animation graph '{graph_name}' cartesian blend2d contains duplicate lattice cell"
            ));
        }
        grid[cell] = sample_index;
    }
    if grid.contains(&usize::MAX) {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d lattice contains a missing cell"
        ));
    }
    Ok(CompiledBlend2DDomain::Cartesian {
        x_values,
        y_values,
        grid,
    })
}

pub(super) fn normalize_angle_radians(value: f32) -> f32 {
    value.rem_euclid(std::f32::consts::TAU)
}

fn compile_directional_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    let mut center_sample = None;
    let mut ring = Vec::with_capacity(samples.len());
    for (sample_index, sample) in samples.iter().enumerate() {
        let radius = sample.position[0].hypot(sample.position[1]);
        if radius <= BLEND2D_POINT_EPSILON {
            if center_sample.replace(sample_index).is_some() {
                return Err(format!(
                    "animation graph '{graph_name}' directional blend2d contains multiple center samples"
                ));
            }
            continue;
        }
        ring.push(CompiledDirectionalSample {
            sample_index,
            angle_radians: normalize_angle_radians(sample.position[1].atan2(sample.position[0])),
            radius,
        });
    }
    if ring.len() < 3 {
        return Err(format!(
            "animation graph '{graph_name}' directional blend2d requires at least three non-center directions"
        ));
    }
    ring.sort_by(|a, b| {
        a.angle_radians
            .total_cmp(&b.angle_radians)
            .then_with(|| a.sample_index.cmp(&b.sample_index))
    });
    for index in 0..ring.len() {
        let a = ring[index].angle_radians;
        let b = if index + 1 == ring.len() {
            ring[0].angle_radians + std::f32::consts::TAU
        } else {
            ring[index + 1].angle_radians
        };
        if b - a <= 1.0e-4 {
            return Err(format!(
                "animation graph '{graph_name}' directional blend2d contains duplicate direction angle"
            ));
        }
    }
    Ok(CompiledBlend2DDomain::Directional {
        center_sample,
        ring,
    })
}

#[derive(Clone, Copy, Debug)]
struct Blend2DTriangleWork {
    vertices: [usize; 3],
}

fn orient2d(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn ccw_triangle(a: usize, b: usize, c: usize, points: &[[f64; 2]]) -> Option<Blend2DTriangleWork> {
    let orientation = orient2d(points[a], points[b], points[c]);
    if orientation.abs() <= 1.0e-12 {
        return None;
    }
    Some(if orientation > 0.0 {
        Blend2DTriangleWork {
            vertices: [a, b, c],
        }
    } else {
        Blend2DTriangleWork {
            vertices: [a, c, b],
        }
    })
}

fn circumcircle_contains(
    triangle: Blend2DTriangleWork,
    point_index: usize,
    points: &[[f64; 2]],
) -> bool {
    let [ia, ib, ic] = triangle.vertices;
    let p = points[point_index];
    let a = [points[ia][0] - p[0], points[ia][1] - p[1]];
    let b = [points[ib][0] - p[0], points[ib][1] - p[1]];
    let c = [points[ic][0] - p[0], points[ic][1] - p[1]];
    let aa = a[0] * a[0] + a[1] * a[1];
    let bb = b[0] * b[0] + b[1] * b[1];
    let cc = c[0] * c[0] + c[1] * c[1];
    let determinant = aa * (b[0] * c[1] - b[1] * c[0]) - bb * (a[0] * c[1] - a[1] * c[0])
        + cc * (a[0] * b[1] - a[1] * b[0]);
    determinant > 1.0e-10
}

fn compile_triangulated_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    if samples.len() < 3 {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d requires at least three samples"
        ));
    }
    let mut points = samples
        .iter()
        .map(|sample| [f64::from(sample.position[0]), f64::from(sample.position[1])])
        .collect::<Vec<_>>();
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (max_x - min_x).max(max_y - min_y);
    if span <= 1.0e-10 {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d domain is degenerate"
        ));
    }
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;
    let super_start = points.len();
    points.push([mid_x - span * 32.0, mid_y - span * 2.0]);
    points.push([mid_x, mid_y + span * 32.0]);
    points.push([mid_x + span * 32.0, mid_y - span * 2.0]);
    let super_triangle = ccw_triangle(super_start, super_start + 1, super_start + 2, &points)
        .ok_or_else(|| format!("animation graph '{graph_name}' blend2d super triangle failed"))?;
    let mut triangles = vec![super_triangle];
    let mut insertion_order = (0..samples.len()).collect::<Vec<_>>();
    insertion_order.sort_by(|left, right| {
        samples[*left].position[0]
            .total_cmp(&samples[*right].position[0])
            .then_with(|| samples[*left].position[1].total_cmp(&samples[*right].position[1]))
            .then_with(|| left.cmp(right))
    });
    for point_index in insertion_order {
        let mut edge_counts = HashMap::<(usize, usize), usize>::new();
        let mut bad = vec![false; triangles.len()];
        for (triangle_index, triangle) in triangles.iter().copied().enumerate() {
            if !circumcircle_contains(triangle, point_index, &points) {
                continue;
            }
            bad[triangle_index] = true;
            let [a, b, c] = triangle.vertices;
            for (left, right) in [(a, b), (b, c), (c, a)] {
                let edge = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                *edge_counts.entry(edge).or_default() += 1;
            }
        }
        let mut next_triangles = triangles
            .into_iter()
            .enumerate()
            .filter_map(|(index, triangle)| (!bad[index]).then_some(triangle))
            .collect::<Vec<_>>();
        let mut boundary = edge_counts
            .into_iter()
            .filter_map(|(edge, count)| (count == 1).then_some(edge))
            .collect::<Vec<_>>();
        boundary.sort_unstable();
        for (left, right) in boundary {
            if let Some(triangle) = ccw_triangle(left, right, point_index, &points) {
                next_triangles.push(triangle);
            }
        }
        triangles = next_triangles;
    }
    let mut compiled = triangles
        .into_iter()
        .filter(|triangle| triangle.vertices.iter().all(|vertex| *vertex < super_start))
        .map(|triangle| CompiledBlendTriangle {
            samples: triangle.vertices,
        })
        .collect::<Vec<_>>();
    if compiled.is_empty() {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d samples are collinear"
        ));
    }
    compiled.sort_by_key(|triangle| {
        let mut key = triangle.samples;
        key.sort_unstable();
        key
    });
    compiled.dedup_by(|left, right| {
        let mut left_key = left.samples;
        let mut right_key = right.samples;
        left_key.sort_unstable();
        right_key.sort_unstable();
        left_key == right_key
    });
    Ok(CompiledBlend2DDomain::Triangulated {
        triangles: compiled,
    })
}

pub(super) fn compile_blend2d_domain(
    graph_name: &str,
    mode: AnimationBlend2DMode,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    validate_blend2d_positions(graph_name, samples)?;
    match mode {
        AnimationBlend2DMode::Cartesian => compile_cartesian_blend2d_domain(graph_name, samples),
        AnimationBlend2DMode::Directional => {
            compile_directional_blend2d_domain(graph_name, samples)
        }
        AnimationBlend2DMode::Triangulated => {
            compile_triangulated_blend2d_domain(graph_name, samples)
        }
    }
}
