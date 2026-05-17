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

const float NE_EPS = 1.0e-5;

float saturate(float v) { return clamp(v, 0.0, 1.0); }
vec3 saturate3(vec3 v) { return clamp(v, vec3(0.0), vec3(1.0)); }
float ne_luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
vec3 ne_safe_hdr(vec3 c) { return max(c, vec3(0.0)); }

float ne_hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 ne_texel_size() {
    ivec2 sz = textureSize(u_scene_hdr, 0);
    return 1.0 / vec2(max(sz.x, 1), max(sz.y, 1));
}

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

vec3 ne_display_encode(vec3 ldr) {
    float gamma = max(pc.display_params.y, 0.1);
    if (abs(gamma - 2.2) < 0.05) {
        return ne_linear_to_display_srgb(ldr);
    }
    return pow(clamp(ldr, vec3(0.0), vec3(1.0)), vec3(1.0 / gamma));
}

vec3 ne_bloom_extract(vec3 c) {
    float peak = max(max(c.r, c.g), c.b);
    float mask = smoothstep(0.85, 2.20, peak);
    return c * mask;
}

vec3 ne_soft_bloom(vec2 uv) {
    vec2 t = ne_texel_size();
    const vec2 offsets[12] = vec2[12](
        vec2( 1.0,  0.0), vec2(-1.0,  0.0), vec2( 0.0,  1.0), vec2( 0.0, -1.0),
        vec2( 1.0,  1.0), vec2(-1.0,  1.0), vec2( 1.0, -1.0), vec2(-1.0, -1.0),
        vec2( 2.0,  0.5), vec2(-2.0, -0.5), vec2( 0.5,  2.0), vec2(-0.5, -2.0)
    );
    const float weights[12] = float[12](
        0.105, 0.105, 0.105, 0.105,
        0.070, 0.070, 0.070, 0.070,
        0.050, 0.050, 0.050, 0.050
    );

    vec3 bloom = vec3(0.0);
    float wsum = 0.0;
    for (int i = 0; i < 12; ++i) {
        vec2 o = offsets[i];
        float w = weights[i];
        bloom += ne_bloom_extract(texture(u_scene_hdr, uv + o * t * 2.25).rgb) * w;
        bloom += ne_bloom_extract(texture(u_scene_hdr, uv + o * t * 6.50).rgb) * (w * 0.42);
        wsum += w * 1.42;
    }
    return bloom / max(wsum, NE_EPS);
}

vec3 ne_local_contrast(vec2 uv, vec3 hdr) {
    vec2 t = ne_texel_size();
    vec3 blur = texture(u_scene_hdr, uv + vec2( t.x, 0.0)).rgb;
    blur += texture(u_scene_hdr, uv + vec2(-t.x, 0.0)).rgb;
    blur += texture(u_scene_hdr, uv + vec2(0.0,  t.y)).rgb;
    blur += texture(u_scene_hdr, uv + vec2(0.0, -t.y)).rgb;
    blur *= 0.25;
    float edge_guard = 1.0 - smoothstep(1.2, 4.5, ne_luma(hdr));
    return max(hdr + (hdr - blur) * (0.055 * edge_guard), vec3(0.0));
}

vec3 ne_natural_pregrade(vec3 hdr, vec2 uv) {
    hdr = ne_safe_hdr(hdr);
    float y = ne_luma(hdr);
    float sat = mix(1.045, 1.075, saturate(y / 3.0));
    hdr = mix(vec3(y), hdr, sat);
    hdr *= vec3(1.015, 1.005, 0.985);

    vec2 d = uv - vec2(0.5);
    float vignette = mix(0.90, 1.0, 1.0 - smoothstep(0.16, 0.72, dot(d, d)));
    hdr *= vignette;
    return max(hdr, vec3(0.0));
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
    float disk = 1.0 - smoothstep(disk_radius * 0.35, disk_radius, d);
    float halo = exp(-d * 20.0) * 0.34 + exp(-d * 4.2) * 0.035;
    float rays = ne_radial_ray(uv, sun_pos) * ray_strength * center_alignment;

    float ghost = 0.0;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.34, 0.030) * 0.80;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.82, 0.055) * 0.45;
    ghost += ne_lens_ghost(uv, sun_pos, center, 1.45, 0.080) * 0.25;
    ghost *= flare_strength * center_alignment;

    vec3 ghost_tint = mix(sun_color, sun_color * vec3(1.10, 0.86, 0.58), 0.42);
    float optical_power = effect_visibility * clamp(sun_intensity / 3.2, 0.0, 2.0);
    return optical_power * (sun_color * (disk * 4.0 + halo + rays) + ghost_tint * ghost);
}

void main() {
    vec2 uv = clamp(v_uv, vec2(0.0), vec2(1.0));
    vec3 hdr = texture(u_scene_hdr, uv).rgb;

    // Single-pass production root: exposure, local contrast, soft bloom, sun optics,
    // natural color grade, tonemap, display encode and deterministic dither.
    hdr = ne_local_contrast(uv, hdr);
    hdr += ne_soft_bloom(uv) * 0.075;
    hdr += ne_sun_optics(uv);
    hdr = max(hdr * max(pc.display_params.x, 0.0) + vec3(pc.display_params.z), vec3(0.0));
    hdr = ne_natural_pregrade(hdr, uv);

    vec3 ldr = pc.display_params.w < 0.5
        ? ne_tonemap_aces_approx(hdr)
        : pc.display_params.w < 1.5
            ? ne_tonemap_reinhard(hdr)
            : clamp(hdr, vec3(0.0), vec3(1.0));

    vec3 display = ne_display_encode(ldr);
    float dither = (ne_hash12(gl_FragCoord.xy) - 0.5) / 255.0;
    o_color = vec4(clamp(display + vec3(dither), vec3(0.0), vec3(1.0)), 1.0);
}
