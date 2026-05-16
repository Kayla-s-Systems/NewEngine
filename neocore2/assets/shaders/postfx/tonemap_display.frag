#version 450

layout(set = 0, binding = 0) uniform sampler2D u_scene_hdr;

layout(push_constant) uniform ToneMapPushConstants {
    float exposure;
    float gamma;
    float black_lift;
    float operator_id;
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

void main() {
    vec3 hdr = texture(u_scene_hdr, v_uv).rgb;
    hdr = max(hdr * max(pc.exposure, 0.0) + vec3(pc.black_lift), vec3(0.0));

    vec3 ldr = pc.operator_id < 0.5
        ? ne_tonemap_aces_approx(hdr)
        : pc.operator_id < 1.5
            ? ne_tonemap_reinhard(hdr)
            : clamp(hdr, vec3(0.0), vec3(1.0));

    o_color = vec4(ne_linear_to_display_srgb(ldr), 1.0);
}
