#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{AntiAliasingMode, PostFxFrameParams, SunPostFxParams};
use newengine_math::{Mat4, Vec3, Vec4};

use super::lights;

/// Apparent angular half-radius of the Sun as seen from Earth (~0.2666 degrees).
const SOLAR_ANGULAR_RADIUS_RAD: f32 = 0.004_653;
const SUN_PROJECTION_DISTANCE: f32 = 2_048.0;

pub(super) fn game_sun_postfx_params(
    world: &newengine_ecs::World,
    viewproj: Mat4,
    camera_position: Vec3,
) -> PostFxFrameParams {
    let mut params = PostFxFrameParams::default();
    let launch_graphics = newengine_core::startup_launch_settings().graphics;
    params.quality.anti_aliasing = match launch_graphics.msaa_samples {
        8 => AntiAliasingMode::Msaa8x,
        4 => AntiAliasingMode::Msaa4x,
        2 => AntiAliasingMode::Msaa2x,
        _ if launch_graphics.taa_enabled => AntiAliasingMode::Taa,
        _ if launch_graphics.fxaa_enabled => AntiAliasingMode::Fxaa,
        _ => AntiAliasingMode::None,
    };
    params.quality.fxaa.enabled = launch_graphics.fxaa_enabled;
    params.quality.fxaa.edge_threshold = launch_graphics.fxaa_edge_threshold;
    params.quality.fxaa.edge_threshold_min = launch_graphics.fxaa_edge_threshold_min;
    params.quality.fxaa.subpixel_quality = launch_graphics.fxaa_subpixel_quality;
    params.quality.taa.enabled = launch_graphics.taa_enabled;
    params.quality.taa.feedback = launch_graphics.taa_feedback;
    params.quality.taa.neighborhood_clamping = launch_graphics.taa_neighborhood_clamping;
    params.quality.taa.jitter_scale = launch_graphics.taa_jitter_scale;
    params.quality.ssao.enabled = launch_graphics.ssao_enabled;
    params.quality.ssao.radius_ws = launch_graphics.ssao_radius_ws;
    params.quality.ssao.intensity = launch_graphics.ssao_intensity;
    params.quality.ssao.quality_steps = launch_graphics.ssao_quality_steps;
    params.quality.ssao.half_resolution = launch_graphics.ssao_half_resolution;

    // Contact shadows belong to the shadow authoring policy, not to the Vulkan
    // backend. Bridge the scene-level ShadowSettings into the renderer-facing
    // postfx DTO so the screen-space contact layer tracks the same authored
    // strength as CSM/PCSS instead of using a backend hard-coded constant.
    let shadow_settings = world
        .resource::<newengine_lighting::ShadowSettings>()
        .copied()
        .unwrap_or_default()
        .sanitized();
    params.quality.contact_shadows.enabled =
        shadow_settings.enabled && shadow_settings.contact_strength > 0.0;
    params.quality.contact_shadows.strength = shadow_settings.contact_strength;

    let sky_postfx = world
        .resource::<crate::gameplay::EnvironmentPostFxState>()
        .copied()
        .unwrap_or_default();
    params.display.exposure = sky_postfx.exposure;
    params.display.gamma = sky_postfx.gamma;
    params.display.black_lift = sky_postfx.black_lift;
    params.quality.color.saturation = sky_postfx.saturation;
    params.quality.color.contrast = sky_postfx.contrast;
    params.quality.color.temperature = sky_postfx.temperature;
    params.quality.color.vignette_strength = sky_postfx.vignette_strength;
    params.quality.color.local_contrast_strength = sky_postfx.local_contrast_strength;
    params.quality.color.dither_strength = sky_postfx.dither_strength;
    params.quality.bloom.enabled = launch_graphics.bloom_enabled;
    params.quality.bloom.threshold = launch_graphics.bloom_threshold;
    params.quality.bloom.knee = launch_graphics.bloom_knee;
    params.quality.bloom.intensity = launch_graphics.bloom_intensity;
    params.quality.bloom.radius = launch_graphics.bloom_radius;

    let Some(sun) = lights::primary_directional_light(world) else {
        return params;
    };

    let incoming = Vec3::new(
        sun.direction_ws[0],
        sun.direction_ws[1],
        sun.direction_ws[2],
    )
    .normalize_or_zero();
    if incoming.length_squared() <= 1.0e-8 || sun.intensity <= 0.0 {
        return params;
    }

    // DirectionalLight.direction_ws points from the Sun into the scene. The
    // visible solar disc lies in the opposite direction from the camera.
    let to_sun = -incoming;
    let Some(screen) = project_direction_to_screen(viewproj, camera_position, to_sun) else {
        return params;
    };
    let screen_x = screen[0];
    let screen_y = screen[1];

    // Fade only at the viewport boundary. Do not use center alignment as an
    // artificial visibility term: a real lens still flares near the frame edge.
    let edge_distance = screen_x
        .min(1.0 - screen_x)
        .min(screen_y)
        .min(1.0 - screen_y);
    let edge_visibility = ((edge_distance + 0.06) / 0.10).clamp(0.0, 1.0);
    let on_screen = (-0.06..=1.06).contains(&screen_x) && (-0.06..=1.06).contains(&screen_y);
    let daylight = ((to_sun.y + 0.035) / 0.16).clamp(0.0, 1.0);
    let horizon_grazing = (1.0 - to_sun.y.abs()).clamp(0.0, 1.0);
    let visibility = if on_screen {
        daylight * edge_visibility
    } else {
        0.0
    };

    // Derive the disc radius from the active projection rather than hard-coding
    // a screen-space size. This keeps the visual Sun stable across FOV/aspect.
    let disk_radius = projected_solar_radius(viewproj, camera_position, to_sun)
        .unwrap_or(0.0045)
        .clamp(0.0015, 0.018);

    params.sun = SunPostFxParams {
        screen_position: [screen_x, screen_y],
        color: sun.color,
        direction: [incoming.x, incoming.y, incoming.z],
        intensity: sun.intensity,
        visibility,
        disk_radius,
        // Lens flare is an optical response to a visible HDR solar source and is
        // intentionally independent from the optional god-ray/streak toggle.
        flare_strength: (0.26 + 0.22 * horizon_grazing) * sky_postfx.sun_glare_scale,
        ray_strength: if launch_graphics.sun_rays_enabled {
            (0.10 + 0.18 * horizon_grazing) * sky_postfx.sun_ray_scale
        } else {
            0.0
        },
    };
    params
}

fn project_direction_to_screen(
    viewproj: Mat4,
    camera_position: Vec3,
    direction: Vec3,
) -> Option<[f32; 2]> {
    let direction = direction.normalize_or_zero();
    if direction.length_squared() <= 1.0e-8 {
        return None;
    }
    let world = camera_position + direction * SUN_PROJECTION_DISTANCE;
    let clip = viewproj * Vec4::new(world.x, world.y, world.z, 1.0);
    if !clip.is_finite() || clip.w <= 1.0e-5 {
        return None;
    }
    let inv_w = 1.0 / clip.w;
    Some([clip.x * inv_w * 0.5 + 0.5, clip.y * inv_w * 0.5 + 0.5])
}

fn projected_solar_radius(viewproj: Mat4, camera_position: Vec3, to_sun: Vec3) -> Option<f32> {
    let to_sun = to_sun.normalize_or_zero();
    let center = project_direction_to_screen(viewproj, camera_position, to_sun)?;

    let mut tangent = to_sun.cross(Vec3::Y).normalize_or_zero();
    if tangent.length_squared() <= 1.0e-8 {
        tangent = to_sun.cross(Vec3::X).normalize_or_zero();
    }
    if tangent.length_squared() <= 1.0e-8 {
        return None;
    }

    let edge_direction = (to_sun * SOLAR_ANGULAR_RADIUS_RAD.cos()
        + tangent * SOLAR_ANGULAR_RADIUS_RAD.sin())
    .normalize_or_zero();
    let edge = project_direction_to_screen(viewproj, camera_position, edge_direction)?;
    let dx = edge[0] - center[0];
    let dy = edge[1] - center[1];
    Some(dx.hypot(dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_solar_radius_is_positive_and_small() {
        let camera = Vec3::ZERO;
        let view = Mat4::IDENTITY;
        let proj = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 5_000.0);
        let radius = projected_solar_radius(proj * view, camera, Vec3::new(0.0, 0.1, -1.0))
            .expect("sun must project");
        assert!(radius > 0.001, "radius={radius}");
        assert!(radius < 0.02, "radius={radius}");
    }

    #[test]
    fn projected_sun_center_tracks_view_projection() {
        let proj = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 0.1, 5_000.0);
        let center = project_direction_to_screen(proj, Vec3::ZERO, -Vec3::Z)
            .expect("forward sun must project");
        assert!((center[0] - 0.5).abs() < 1.0e-4, "x={}", center[0]);
        assert!((center[1] - 0.5).abs() < 1.0e-4, "y={}", center[1]);
    }
}
