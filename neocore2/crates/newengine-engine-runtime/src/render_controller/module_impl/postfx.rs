#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{PostFxFrameParams, SunPostFxParams};
use newengine_math::{Vec3, Vec4};

use super::lights;

pub(super) fn game_sun_postfx_params(
    world: &newengine_ecs::World,
    viewproj: newengine_math::Mat4,
    camera_position: Vec3,
) -> PostFxFrameParams {
    let mut params = PostFxFrameParams::default();
    let sky_postfx = world
        .resource::<crate::scene_bridge::SkyPostFxRuntime>()
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
    params.quality.bloom.enabled = sky_postfx.bloom_intensity > 0.001;
    params.quality.bloom.threshold = sky_postfx.bloom_threshold;
    params.quality.bloom.knee = sky_postfx.bloom_knee;
    params.quality.bloom.intensity = sky_postfx.bloom_intensity;
    params.quality.bloom.radius = sky_postfx.bloom_radius;

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

    // DirectionalLight.direction_ws points from the sun into the scene. The
    // visible solar disk lies in the opposite direction from the camera.
    let to_sun = -incoming;
    let sun_world = camera_position + to_sun * 2048.0;
    let clip = viewproj * Vec4::new(sun_world.x, sun_world.y, sun_world.z, 1.0);
    if clip.w <= 1.0e-5 {
        return params;
    }

    let inv_w = 1.0 / clip.w;
    let ndc_x = clip.x * inv_w;
    let ndc_y = clip.y * inv_w;
    let ndc_z = clip.z * inv_w;
    let screen_x = ndc_x * 0.5 + 0.5;
    let screen_y = ndc_y * 0.5 + 0.5;

    let on_screen =
        (-0.18..=1.18).contains(&screen_x) && (-0.18..=1.18).contains(&screen_y) && ndc_z >= -1.0;
    let center_alignment = (1.0 - ((screen_x - 0.5).hypot(screen_y - 0.5) * 1.72)).clamp(0.0, 1.0);
    let horizon_grazing = (1.0 - to_sun.y.abs()).clamp(0.0, 1.0);
    let daylight = ((to_sun.y + 0.07) / 0.24).clamp(0.0, 1.0);
    let visibility = if on_screen {
        center_alignment.max(0.12) * daylight
    } else {
        0.0
    };

    params.sun = SunPostFxParams {
        screen_position: [screen_x, screen_y],
        color: sun.color,
        direction: [incoming.x, incoming.y, incoming.z],
        intensity: sun.intensity,
        visibility,
        disk_radius: 0.013 + 0.012 * horizon_grazing,
        flare_strength: (0.18 + 0.32 * horizon_grazing) * sky_postfx.sun_glare_scale,
        ray_strength: (0.14 + 0.30 * horizon_grazing) * sky_postfx.sun_ray_scale,
    };
    params
}
