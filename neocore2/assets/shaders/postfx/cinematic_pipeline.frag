#version 450

// NewEngine cinematic post stack.
// Native GLSL translation inspired by MXAO depth-neighbourhood sampling,
// CinematicDOF bokeh gathers, ENB lens ghost/streak layers, filmic tone mapping,
// and deterministic post-scan/dither. It is intentionally self-contained so the
// existing one-input PostFX native pass can use it immediately through ShaderRegistry.

layout(set = 0, binding = 0) uniform sampler2D u_scene_hdr;

layout(push_constant) uniform PostFxPushConstants {
    vec4 display_params;      // x adapted exposure, y gamma, z black lift, w operator id
    vec4 sun_screen;          // x/y normalized screen pos, z visibility, w intensity
    vec4 sun_color_radius;    // rgb linear color, w disk radius
    vec4 sun_effects;         // x flare strength, y ray strength, z focus distance, w dof blur px
    vec4 bloom_params;        // x threshold, y soft knee, z intensity, w radius multiplier
    vec4 fxaa_params;         // x enabled, y edge threshold, z min threshold, w subpixel quality
    vec4 color_params;        // x saturation, y contrast, z temperature, w vignette strength
    vec4 post_params;         // x local contrast, y dither strength, z aa mode, w taa feedback
} pc;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

const float NE_EPS = 1.0e-5;
const float NE_GOLDEN_ANGLE = 2.39996323;

float saturate(float v) { return clamp(v, 0.0, 1.0); }
vec3 saturate3(vec3 v) { return clamp(v, vec3(0.0), vec3(1.0)); }
float ne_luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
vec3 ne_safe_hdr(vec3 c) { return clamp(c, vec3(0.0), vec3(65504.0)); }

vec2 ne_texel_size() {
    ivec2 sz = textureSize(u_scene_hdr, 0);
    return 1.0 / vec2(max(sz.x, 1), max(sz.y, 1));
}

vec3 ne_read_hdr(vec2 uv) {
    return ne_safe_hdr(texture(u_scene_hdr, clamp(uv, vec2(0.0), vec2(1.0))).rgb);
}

float ne_hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 ne_vogel_disk(int i, int count) {
    float r = sqrt((float(i) + 0.5) / max(float(count), 1.0));
    float a = float(i) * NE_GOLDEN_ANGLE;
    return vec2(cos(a), sin(a)) * r;
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

vec3 ne_fxaa_hdr(vec2 uv) {
    if (pc.fxaa_params.x < 0.5 || pc.post_params.z < 0.5 || pc.post_params.z > 1.5) {
        return ne_read_hdr(uv);
    }

    vec2 t = ne_texel_size();
    vec3 rgb_m  = ne_read_hdr(uv);
    vec3 rgb_n  = ne_read_hdr(uv + vec2(0.0, -t.y));
    vec3 rgb_s  = ne_read_hdr(uv + vec2(0.0,  t.y));
    vec3 rgb_w  = ne_read_hdr(uv + vec2(-t.x, 0.0));
    vec3 rgb_e  = ne_read_hdr(uv + vec2( t.x, 0.0));
    vec3 rgb_nw = ne_read_hdr(uv + vec2(-t.x, -t.y));
    vec3 rgb_ne = ne_read_hdr(uv + vec2( t.x, -t.y));
    vec3 rgb_sw = ne_read_hdr(uv + vec2(-t.x,  t.y));
    vec3 rgb_se = ne_read_hdr(uv + vec2( t.x,  t.y));

    float l_m  = ne_luma(rgb_m);
    float l_n  = ne_luma(rgb_n);
    float l_s  = ne_luma(rgb_s);
    float l_w  = ne_luma(rgb_w);
    float l_e  = ne_luma(rgb_e);
    float l_nw = ne_luma(rgb_nw);
    float l_ne = ne_luma(rgb_ne);
    float l_sw = ne_luma(rgb_sw);
    float l_se = ne_luma(rgb_se);

    float l_min = min(l_m, min(min(l_n, l_s), min(l_w, l_e)));
    float l_max = max(l_m, max(max(l_n, l_s), max(l_w, l_e)));
    float range = l_max - l_min;
    float threshold = max(pc.fxaa_params.z, l_max * pc.fxaa_params.y);
    if (range < threshold) {
        return rgb_m;
    }

    vec2 dir;
    dir.x = -((l_nw + l_ne) - (l_sw + l_se));
    dir.y =  ((l_nw + l_sw) - (l_ne + l_se));
    float dir_reduce = max((l_n + l_s + l_w + l_e) * 0.03125, 1.0 / 128.0);
    float inv_dir = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(dir * inv_dir, vec2(-8.0), vec2(8.0)) * t;

    vec3 rgb_a = 0.5 * (ne_read_hdr(uv + dir * (1.0 / 3.0 - 0.5)) + ne_read_hdr(uv + dir * (2.0 / 3.0 - 0.5)));
    vec3 rgb_b = rgb_a * 0.5 + 0.25 * (ne_read_hdr(uv + dir * -0.5) + ne_read_hdr(uv + dir * 0.5));
    float l_b = ne_luma(rgb_b);
    vec3 fxaa = (l_b < l_min || l_b > l_max) ? rgb_a : rgb_b;

    return mix(rgb_m, fxaa, clamp(pc.fxaa_params.w, 0.0, 1.0));
}

float ne_mxao_luma_occlusion(vec2 uv, vec3 center_hdr) {
    // Depth is not bound in the current native pass yet, so this is the bridge path:
    // use luminance curvature as a cheap MXAO-style contact darkening signal until
    // GBuffer depth/normal descriptors are connected to the native SSAO pass.
    vec2 t = ne_texel_size();
    float center = ne_luma(center_hdr);
    float radius = mix(1.25, 3.25, clamp(pc.color_params.w + pc.post_params.x * 4.0, 0.0, 1.0));
    float occ = 0.0;
    float wsum = 0.0;
    for (int i = 0; i < 12; ++i) {
        vec2 dir = ne_vogel_disk(i, 12);
        float w = 1.0 - float(i) / 12.0;
        float sample_luma = ne_luma(ne_read_hdr(uv + dir * t * radius));
        float cavity = max(center - sample_luma, 0.0);
        occ += smoothstep(0.015, 0.18, cavity) * w;
        wsum += w;
    }
    float strength = clamp(0.10 + pc.post_params.x * 1.45, 0.0, 0.38);
    return 1.0 - strength * clamp(occ / max(wsum, NE_EPS), 0.0, 1.0);
}

vec3 ne_bloom_extract(vec3 c) {
    c = ne_safe_hdr(c);
    float threshold = max(pc.bloom_params.x, 0.0);
    float knee = max(pc.bloom_params.y, 1.0e-4);
    float peak = max(max(c.r, c.g), c.b);
    float mask = smoothstep(threshold, threshold + knee, peak);
    return c * mask;
}

vec3 ne_soft_bloom(vec2 uv) {
    float intensity = max(pc.bloom_params.z, 0.0);
    if (intensity <= 0.0001) {
        return vec3(0.0);
    }

    vec2 t = ne_texel_size();
    float radius = clamp(pc.bloom_params.w, 0.25, 5.0);
    vec3 bloom = vec3(0.0);
    float wsum = 0.0;
    for (int i = 0; i < 14; ++i) {
        vec2 o = ne_vogel_disk(i, 14);
        float w = 1.0 - float(i) / 18.0;
        bloom += ne_bloom_extract(ne_read_hdr(uv + o * t * (2.0 * radius))) * w;
        bloom += ne_bloom_extract(ne_read_hdr(uv + o * t * (7.5 * radius))) * (w * 0.38);
        wsum += w * 1.38;
    }
    return (bloom / max(wsum, NE_EPS)) * intensity;
}

vec3 ne_cinematic_dof(vec2 uv, vec3 hdr) {
    float blur_px = clamp(pc.sun_effects.w, 0.0, 18.0);
    if (blur_px <= 0.05) {
        return hdr;
    }

    vec2 t = ne_texel_size();
    vec2 center = vec2(0.5);
    float radial = smoothstep(0.10, 0.72, length(uv - center));
    float focus_weight = clamp(blur_px / 18.0, 0.0, 1.0) * radial;
    vec3 bokeh = hdr;
    float wsum = 1.0;
    for (int i = 0; i < 18; ++i) {
        vec2 d = ne_vogel_disk(i, 18);
        float ring = length(d);
        float w = mix(1.0, 0.35, ring);
        vec3 tap = ne_read_hdr(uv + d * t * blur_px * 1.35);
        float highlight = smoothstep(1.0, 6.0, ne_luma(tap));
        bokeh += tap * (w + highlight * 1.15);
        wsum += w + highlight * 1.15;
    }
    bokeh /= max(wsum, NE_EPS);
    float edge_guard = 1.0 - smoothstep(0.18, 0.75, length(fwidth(hdr)));
    return mix(hdr, bokeh, focus_weight * edge_guard);
}

vec3 ne_local_contrast(vec2 uv, vec3 hdr) {
    float strength = clamp(pc.post_params.x, 0.0, 0.25);
    if (strength <= 0.0001) {
        return hdr;
    }
    vec2 t = ne_texel_size();
    vec3 blur = ne_read_hdr(uv + vec2( t.x, 0.0));
    blur += ne_read_hdr(uv + vec2(-t.x, 0.0));
    blur += ne_read_hdr(uv + vec2(0.0,  t.y));
    blur += ne_read_hdr(uv + vec2(0.0, -t.y));
    blur *= 0.25;
    float edge_guard = 1.0 - smoothstep(1.2, 4.5, ne_luma(hdr));
    return max(hdr + (hdr - blur) * (strength * edge_guard), vec3(0.0));
}

vec3 ne_natural_pregrade(vec3 hdr, vec2 uv) {
    hdr = ne_safe_hdr(hdr);
    float y = ne_luma(hdr);
    float sat = clamp(pc.color_params.x, 0.0, 2.5);
    hdr = mix(vec3(y), hdr, sat);

    float contrast = clamp(pc.color_params.y, 0.2, 2.5);
    hdr = max((hdr - vec3(0.18)) * contrast + vec3(0.18), vec3(0.0));

    float temperature = clamp(pc.color_params.z, -1.0, 1.0);
    vec3 warm = vec3(1.035, 1.005, 0.965);
    vec3 cool = vec3(0.965, 1.005, 1.045);
    hdr *= mix(vec3(1.0), temperature >= 0.0 ? warm : cool, abs(temperature));

    vec2 d = uv - vec2(0.5);
    float vignette_strength = clamp(pc.color_params.w, 0.0, 0.85);
    float vignette = mix(1.0 - vignette_strength, 1.0, 1.0 - smoothstep(0.16, 0.72, dot(d, d)));
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

vec3 ne_enb_lens_artefacts(vec2 uv) {
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
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.28, 0.026) * 0.80;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.63, 0.045) * 0.60;
    ghost += ne_lens_ghost(uv, sun_pos, center, 0.98, 0.058) * 0.38;
    ghost += ne_lens_ghost(uv, sun_pos, center, 1.48, 0.084) * 0.24;
    ghost *= flare_strength * center_alignment;

    vec3 ghost_tint = mix(sun_color, sun_color * vec3(1.10, 0.86, 0.58), 0.42);
    float optical_power = effect_visibility * clamp(sun_intensity / 3.2, 0.0, 2.0);
    return optical_power * (sun_color * (disk * 4.0 + halo + rays) + ghost_tint * ghost);
}

void main() {
    vec2 uv = clamp(v_uv, vec2(0.0), vec2(1.0));
    vec3 hdr = ne_fxaa_hdr(uv);

    // Depth/normal data is not bound to this root post pass yet. The old luma-only
    // AO approximation read horizon/tree silhouettes as screen-space cavities and
    // produced a grey field over the playable image. Keep the root post stack
    // neutral until the real depth-backed AO pass is connected.
    hdr = ne_cinematic_dof(uv, hdr);
    hdr = ne_local_contrast(uv, hdr);
    hdr += ne_soft_bloom(uv);
    hdr += ne_enb_lens_artefacts(uv);
    hdr = max(hdr * max(pc.display_params.x, 0.0) + vec3(pc.display_params.z), vec3(0.0));
    hdr = ne_natural_pregrade(hdr, uv);

    vec3 ldr = pc.display_params.w < 0.5
        ? ne_tonemap_aces_approx(hdr)
        : pc.display_params.w < 1.5
            ? ne_tonemap_reinhard(hdr)
            : clamp(hdr, vec3(0.0), vec3(1.0));

    vec3 display = ne_display_encode(ldr);
    float dither_strength = clamp(pc.post_params.y, 0.0, 2.0);
    float dither = (ne_hash12(gl_FragCoord.xy) - 0.5) * dither_strength / 255.0;
    o_color = vec4(clamp(display + vec3(dither), vec3(0.0), vec3(1.0)), 1.0);
}
