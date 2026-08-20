#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RectI32, RenderApi, Viewport};
use newengine_core::EngineResult;
use newengine_lighting::LocalShadowSettings;
use newengine_math::{Mat4, Vec3};
use newengine_render_feature_api::{
    LocalShadowFrame, LocalShadowLightFrame, LocalShadowPlan, LocalShadowViewFrame,
    ShadowCasterCull, ShadowLightKind, MAX_LOCAL_SHADOW_LIGHTS, MAX_LOCAL_SHADOW_VIEWS,
    MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS,
};

use crate::render_controller::RuntimeRenderController;

use super::targets::{ensure_local_shadow_rt, retire_local_shadow_rt};

const LOCAL_ATLAS_MAX_EXTENT: u32 = 8192;

#[derive(Clone, Copy, Debug)]
struct LocalShadowCandidate {
    stable_id: u64,
    light_kind: ShadowLightKind,
    packed_light_index: u32,
    position: Vec3,
    direction: Vec3,
    range: f32,
    outer_angle_rad: f32,
    score: f32,
    resolution: u32,
}

#[inline]
fn kind_rank(kind: ShadowLightKind) -> u8 {
    match kind {
        ShadowLightKind::Point => 0,
        ShadowLightKind::Spot => 1,
        ShadowLightKind::Directional => 2,
    }
}

#[inline]
fn round_down_power_of_two(value: u32) -> u32 {
    if value <= 1 {
        return 1;
    }
    1u32 << (31 - value.leading_zeros())
}

#[inline]
fn choose_resolution(
    settings: LocalShadowSettings,
    range: f32,
    distance: f32,
    intensity: f32,
) -> u32 {
    let coverage = range.max(0.01) / distance.max(0.5);
    let energy = (intensity.max(0.0).sqrt() / 4.0).clamp(0.0, 1.0);
    let importance = coverage * (0.72 + energy * 0.28);
    let half = (settings.max_resolution / 2).max(settings.min_resolution);
    if importance >= 0.42 {
        settings.max_resolution
    } else if importance >= 0.16 {
        half
    } else {
        settings.min_resolution
    }
}

#[inline]
fn candidate_score(range: f32, distance: f32, intensity: f32) -> f32 {
    let coverage = range.max(0.01) / distance.max(0.5);
    coverage * coverage * (1.0 + intensity.max(0.0).sqrt() * 0.25)
}

fn collect_candidates(
    world: &newengine_ecs::World,
    settings: LocalShadowSettings,
    camera: Vec3,
) -> Vec<LocalShadowCandidate> {
    let snapshot = super::super::lights::collect_light_scene_snapshot(world);
    let mut out = Vec::new();

    if settings.point_enabled {
        for (packed_index, point) in snapshot
            .sorted_point_lights()
            .into_iter()
            .take(MAX_POINT_LIGHTS)
            .enumerate()
        {
            let range = point.light.range.max(0.01);
            let distance = point.position.distance(camera);
            if distance - range > settings.max_distance {
                continue;
            }
            out.push(LocalShadowCandidate {
                stable_id: point.stable_id,
                light_kind: ShadowLightKind::Point,
                packed_light_index: packed_index as u32,
                position: point.position,
                direction: Vec3::ZERO,
                range,
                outer_angle_rad: 0.0,
                score: candidate_score(range, distance, point.light.intensity),
                resolution: choose_resolution(settings, range, distance, point.light.intensity),
            });
        }
    }

    if settings.spot_enabled {
        for (packed_index, spot) in snapshot
            .sorted_spot_lights()
            .into_iter()
            .take(MAX_SPOT_LIGHTS)
            .enumerate()
        {
            let range = spot.light.range.max(0.01);
            let distance = spot.position.distance(camera);
            if distance - range > settings.max_distance {
                continue;
            }
            let direction = Vec3::new(
                spot.light.direction_ws[0],
                spot.light.direction_ws[1],
                spot.light.direction_ws[2],
            )
            .normalize_or_zero();
            if direction.length_squared() <= 1.0e-8 {
                continue;
            }
            out.push(LocalShadowCandidate {
                stable_id: spot.stable_id,
                light_kind: ShadowLightKind::Spot,
                packed_light_index: packed_index as u32,
                position: spot.position,
                direction,
                range,
                outer_angle_rad: spot.light.outer_angle_rad.clamp(0.05, 1.50),
                score: candidate_score(range, distance, spot.light.intensity) * 1.08,
                resolution: choose_resolution(settings, range, distance, spot.light.intensity),
            });
        }
    }

    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| kind_rank(a.light_kind).cmp(&kind_rank(b.light_kind)))
            .then_with(|| a.stable_id.cmp(&b.stable_id))
    });
    out.truncate(
        settings
            .max_shadowed_lights
            .min(MAX_LOCAL_SHADOW_LIGHTS as u32) as usize,
    );
    out
}

#[inline]
fn point_face_basis(face: u32) -> (Vec3, Vec3) {
    match face {
        0 => (Vec3::X, -Vec3::Y),
        1 => (-Vec3::X, -Vec3::Y),
        2 => (Vec3::Y, Vec3::Z),
        3 => (-Vec3::Y, -Vec3::Z),
        4 => (Vec3::Z, -Vec3::Y),
        _ => (-Vec3::Z, -Vec3::Y),
    }
}

#[inline]
fn perspective_view(
    position: Vec3,
    direction: Vec3,
    up: Vec3,
    half_fov: f32,
    range: f32,
) -> (Mat4, Mat4, ShadowCasterCull) {
    let near = (range * 0.004).clamp(0.02, 0.20);
    let far = range.max(near + 0.05);
    let view = Mat4::look_at_rh(position, position + direction, up);
    let half_fov = half_fov.clamp(0.05, 1.52);
    let projection = Mat4::perspective_rh((half_fov * 2.0).min(3.04), 1.0, near, far);
    let cull = ShadowCasterCull::perspective(view, half_fov.tan(), near, far);
    (projection * view, view, cull)
}

pub fn build_local_shadow_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
    lit: newengine_material_domain_api::LitPipeline,
    camera_position: [f32; 3],
) -> EngineResult<LocalShadowPlan> {
    let settings = world
        .resource::<LocalShadowSettings>()
        .copied()
        .unwrap_or_default()
        .sanitized();
    if !settings.enabled {
        retire_local_shadow_rt(this);
        return Ok(LocalShadowPlan::disabled(lit.white_texture));
    }

    let camera = Vec3::new(camera_position[0], camera_position[1], camera_position[2]);
    let mut candidates = collect_candidates(world, settings, camera);
    if candidates.is_empty() {
        retire_local_shadow_rt(this);
        return Ok(LocalShadowPlan::disabled(lit.white_texture));
    }

    let total_views = candidates
        .iter()
        .map(|candidate| {
            if candidate.light_kind == ShadowLightKind::Point {
                6usize
            } else {
                1usize
            }
        })
        .sum::<usize>()
        .min(MAX_LOCAL_SHADOW_VIEWS);
    let columns = (total_views as f32).sqrt().ceil().max(1.0) as u32;
    let rows = (total_views as u32).div_ceil(columns).max(1);
    let grid_span = columns.max(rows).max(1);
    let atlas_resolution_cap = round_down_power_of_two(LOCAL_ATLAS_MAX_EXTENT / grid_span)
        .clamp(settings.min_resolution, settings.max_resolution);
    for candidate in &mut candidates {
        candidate.resolution = candidate
            .resolution
            .min(atlas_resolution_cap)
            .max(settings.min_resolution);
    }

    let cell_size = atlas_resolution_cap.max(settings.min_resolution);
    let atlas_extent = Extent2D::new(columns * cell_size, rows * cell_size);
    let Some((target, texture)) = ensure_local_shadow_rt(this, r, atlas_extent)? else {
        return Ok(LocalShadowPlan::disabled(lit.white_texture));
    };

    let mut frame = LocalShadowFrame {
        texture,
        atlas_extent,
        light_count: candidates.len() as u32,
        view_count: total_views as u32,
        lights: [LocalShadowLightFrame::disabled(); MAX_LOCAL_SHADOW_LIGHTS],
        views: [LocalShadowViewFrame::disabled(); MAX_LOCAL_SHADOW_VIEWS],
    };

    let mut view_index = 0usize;
    for (light_slot, candidate) in candidates.iter().enumerate() {
        let first_view = view_index as u32;
        let view_count = if candidate.light_kind == ShadowLightKind::Point {
            6u32
        } else {
            1u32
        };
        frame.lights[light_slot] = LocalShadowLightFrame {
            stable_id: candidate.stable_id,
            light_kind: candidate.light_kind,
            packed_light_index: candidate.packed_light_index,
            first_view,
            view_count,
            resolution: candidate.resolution,
            range: candidate.range,
            bias: settings.bias,
            normal_bias: settings.normal_bias,
            strength: settings.strength,
        };

        for face in 0..view_count {
            if view_index >= MAX_LOCAL_SHADOW_VIEWS {
                break;
            }
            let col = view_index as u32 % columns;
            let row = view_index as u32 / columns;
            let inset = (cell_size - candidate.resolution) / 2;
            let x = col * cell_size + inset;
            let y = row * cell_size + inset;
            let viewport = Viewport {
                x: x as f32,
                y: y as f32,
                w: candidate.resolution as f32,
                h: candidate.resolution as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = RectI32::new(
                x as i32,
                y as i32,
                candidate.resolution as i32,
                candidate.resolution as i32,
            );

            let (light_mvp, caster_cull) = if candidate.light_kind == ShadowLightKind::Point {
                let (direction, up) = point_face_basis(face);
                let (mvp, _view, cull) = perspective_view(
                    candidate.position,
                    direction,
                    up,
                    std::f32::consts::FRAC_PI_4,
                    candidate.range,
                );
                (mvp, cull)
            } else {
                let up = if candidate.direction.dot(Vec3::Y).abs() > 0.94 {
                    Vec3::Z
                } else {
                    Vec3::Y
                };
                let (mvp, _view, cull) = perspective_view(
                    candidate.position,
                    candidate.direction,
                    up,
                    candidate.outer_angle_rad,
                    candidate.range,
                );
                (mvp, cull)
            };

            frame.views[view_index] = LocalShadowViewFrame {
                light_mvp,
                viewport,
                scissor,
                light_slot: light_slot as u32,
                face_index: face,
                resolution: candidate.resolution,
                caster_cull,
            };
            view_index += 1;
        }
    }
    frame.view_count = view_index as u32;
    frame.light_count = candidates.len() as u32;

    Ok(LocalShadowPlan::active(target, frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_budget_is_power_of_two_and_bounded() {
        let settings = LocalShadowSettings::default().sanitized();
        for (range, distance, intensity) in [(3.0, 20.0, 2.0), (8.0, 4.0, 40.0)] {
            let resolution = choose_resolution(settings, range, distance, intensity);
            assert!(resolution.is_power_of_two());
            assert!(resolution >= settings.min_resolution);
            assert!(resolution <= settings.max_resolution);
        }
    }

    #[test]
    fn point_faces_are_six_unique_axes() {
        let mut directions = Vec::new();
        for face in 0..6 {
            directions.push(point_face_basis(face).0);
        }
        for i in 0..directions.len() {
            for j in i + 1..directions.len() {
                assert_ne!(directions[i], directions[j]);
            }
        }
    }
}
