#version 450
#include "common.glsl"

#ifndef FXAA_SUBPIX
#define FXAA_SUBPIX 0.750000
#endif
#ifndef FXAA_EDGE_THRESHOLD
#define FXAA_EDGE_THRESHOLD 0.166000
#endif
#ifndef FXAA_EDGE_THRESHOLD_MIN
#define FXAA_EDGE_THRESHOLD_MIN 0.083300
#endif
#ifndef FXAA_SEARCH_STEPS
#define FXAA_SEARCH_STEPS 12
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_scene);
    vec3 rgb_m = texture(u_scene, v_uv).rgb;
    float luma_m = ne_luma(rgb_m);
    float luma_n = ne_luma(texture(u_scene, v_uv + vec2(0.0, -texel.y)).rgb);
    float luma_s = ne_luma(texture(u_scene, v_uv + vec2(0.0,  texel.y)).rgb);
    float luma_w = ne_luma(texture(u_scene, v_uv + vec2(-texel.x, 0.0)).rgb);
    float luma_e = ne_luma(texture(u_scene, v_uv + vec2( texel.x, 0.0)).rgb);

    float luma_min = min(luma_m, min(min(luma_n, luma_s), min(luma_w, luma_e)));
    float luma_max = max(luma_m, max(max(luma_n, luma_s), max(luma_w, luma_e)));
    float range = luma_max - luma_min;
    if (range < max(float(FXAA_EDGE_THRESHOLD_MIN), luma_max * float(FXAA_EDGE_THRESHOLD))) {
        o_color = vec4(rgb_m, 1.0);
        return;
    }

    float edge_h = abs(luma_w + luma_e - 2.0 * luma_m);
    float edge_v = abs(luma_n + luma_s - 2.0 * luma_m);
    bool horizontal = edge_h >= edge_v;
    vec2 dir = horizontal ? vec2(texel.x, 0.0) : vec2(0.0, texel.y);

    vec3 span = rgb_m;
    float total = 1.0;
    int steps = clamp(int(FXAA_SEARCH_STEPS), 2, 24);
    for (int i = 1; i <= steps; ++i) {
        float w = 1.0 / (1.0 + float(i));
        span += texture(u_scene, v_uv + dir * float(i)).rgb * w;
        span += texture(u_scene, v_uv - dir * float(i)).rgb * w;
        total += 2.0 * w;
    }

    vec3 aa = span / total;
    float subpix = clamp(float(FXAA_SUBPIX), 0.0, 1.0);
    float blend = smoothstep(0.0, 0.25, range) * subpix;
    o_color = vec4(mix(rgb_m, aa, blend), 1.0);
}
