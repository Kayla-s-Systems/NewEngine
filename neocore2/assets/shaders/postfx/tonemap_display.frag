#version 450

layout(set = 0, binding = 0) uniform sampler2D u_scene_hdr;

layout(push_constant) uniform PostFxPushConstants {
    vec4 display_params;      // x exposure, y gamma, z black lift, w operator id
    vec4 sun_screen;          // x/y normalized screen pos, z visibility, w intensity
    vec4 sun_color_radius;    // rgb linear color, w disk radius
    vec4 sun_effects;         // x flare strength, y ray strength, z/w reserved
} pc;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

vec3 ne_safe_hdr(vec3 c) { return max(c, vec3(0.0)); }

vec3 ne_tonemap_reinhard(vec3 c) {
    c = ne_safe_hdr(c);
    return c / (vec3(1.0) + c);
}

vec3 ne_tonemap_aces_approx(vec3 c) {
    c = ne_safe_hdr(c);
    const float a = 2.51;
    const float b = 0.03;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((c * (a * c + b)) / (c * (2.43 * c + d) + e), vec3(0.0), vec3(1.0));
}

vec3 ne_linear_to_display_srgb(vec3 c) {
    c = clamp(c, vec3(0.0), vec3(1.0));
    vec3 lo = c * 12.92;
    vec3 hi = 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055;
    return mix(hi, lo, lessThanEqual(c, vec3(0.0031308)));
}

float ne_lens_ghost(vec2 uv, vec2 sun_pos, vec2 center, float scale, float radius) {
    vec2 ghost_pos = center + (center - sun_pos) * scale;
    float d = length(uv - ghost_pos);
    return pow(clamp(1.0 - d / max(radius, 1.0e-4), 0.0, 1.0), 3.0);
}

float ne_radial_ray(vec2 uv, vec2 sun_pos) {
    vec2 to_px = uv - sun_pos;
    float dist = length(to_px);
    float angle = atan(to_px.y, to_px.x);
    float bands = sin(angle * 18.0) * 0.5 + 0.5;
    bands = mix(bands, sin(angle * 31.0 + dist * 28.0) * 0.5 + 0.5, 0.35);
    float radial = exp(-dist * 2.2);
    return radial * smoothstep(0.42, 1.0, bands);
}

vec3 ne_sun_optics(vec2 uv) {
    float visibility = clamp(pc.sun_screen.z, 0.0, 1.0);
    if (visibility <= 0.001) {
        return vec3(0.0);
    }

    vec2 sun_pos = pc.sun_screen.xy;
    vec2 center = vec2(0.5);
    vec3 sun_color = max(pc.sun_color_radius.rgb, vec3(0.0));
    float sun_intensity = max(pc.sun_screen.w, 0.0);
    float disk_radius = max(pc.sun_color_radius.w, 0.0025);
    float flare_strength = max(pc.sun_effects.x, 0.0);
    float ray_strength = max(pc.sun_effects.y, 0.0);

    float center_alignment = pow(clamp(1.0 - length(sun_pos - center) * 1.85, 0.0, 1.0), 1.35);
    float screen_fade = smoothstep(-0.12, 0.02, sun_pos.x) * smoothstep(-0.12, 0.02, sun_pos.y)
        * smoothstep(-0.12, 0.02, 1.0 - sun_pos.x) * smoothstep(-0.12, 0.02, 1.0 - sun_pos.y);
    float effect_visibility = visibility * screen_fade;

    float d = length(uv - sun_pos);
    float disk = smoothstep(disk_radius, disk_radius * 0.35, d);
    float halo = exp(-d * 22.0) * 0.35;
    float rays = ne_radial_ray(uv, sun_pos) * ray_strength * center_alignment;

    float ghost = 0.0;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.38, 0.035) * 0.70;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.82, 0.055) * 0.45;
    ghost += ne_lens_ghost(uv, sun_pos, center, 1.45, 0.080) * 0.25;
    ghost *= flare_strength * center_alignment;

    float optical_power = effect_visibility * clamp(sun_intensity / 3.2, 0.0, 2.0);
    return sun_color * optical_power * (disk * 4.0 + halo + rays + ghost);
}

void main() {
    vec3 hdr = texture(u_scene_hdr, v_uv).rgb;
    hdr += ne_sun_optics(v_uv);
    hdr = max(hdr * max(pc.display_params.x, 0.0) + vec3(pc.display_params.z), vec3(0.0));

    vec3 ldr = pc.display_params.w < 0.5
        ? ne_tonemap_aces_approx(hdr)
        : pc.display_params.w < 1.5
            ? ne_tonemap_reinhard(hdr)
            : clamp(hdr, vec3(0.0), vec3(1.0));

    o_color = vec4(ne_linear_to_display_srgb(ldr), 1.0);
}
