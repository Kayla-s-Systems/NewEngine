#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    PostFxFrameParams, SunPostFxParams,
};
use newengine_math::{Vec3, Vec4};

use super::lights;

pub(super) fn game_sun_postfx_params(
    world: &newengine_ecs::World,
    viewproj: newengine_math::Mat4,
    camera_position: Vec3,
) -> PostFxFrameParams {
    let mut params = PostFxFrameParams::default();

    let Some(sun) = lights::primary_directional_light(world) else {
        return params;
    };

    let incoming = Vec3::new(sun.direction_ws[0], sun.direction_ws[1], sun.direction_ws[2])
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

    let on_screen = screen_x >= -0.18
        && screen_x <= 1.18
        && screen_y >= -0.18
        && screen_y <= 1.18
        && ndc_z >= -1.0;
    let center_alignment = (1.0 - ((screen_x - 0.5).hypot(screen_y - 0.5) * 1.72))
        .clamp(0.0, 1.0);
    let horizon_grazing = (1.0 - to_sun.y.abs()).clamp(0.0, 1.0);
    let daylight = ((to_sun.y + 0.07) / 0.24).clamp(0.0, 1.0);
    let visibility = if on_screen { center_alignment.max(0.12) * daylight } else { 0.0 };

    params.sun = SunPostFxParams {
        screen_position: [screen_x, screen_y],
        color: sun.color,
        intensity: sun.intensity,
        visibility,
        disk_radius: 0.013 + 0.012 * horizon_grazing,
        flare_strength: 0.18 + 0.32 * horizon_grazing,
        ray_strength: 0.14 + 0.30 * horizon_grazing,
    };
    params
}

