#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef PCSS_BLOCKER_SAMPLES
#define PCSS_BLOCKER_SAMPLES 8
#endif
#ifndef PCSS_FILTER_SAMPLES
#define PCSS_FILTER_SAMPLES 16
#endif
#ifndef PCSS_LIGHT_RADIUS
#define PCSS_LIGHT_RADIUS 0.035000
#endif
#ifndef SHADOW_DEPTH_BIAS
#define SHADOW_DEPTH_BIAS 0.000800
#endif
#ifndef SHADOW_NORMAL_BIAS
#define SHADOW_NORMAL_BIAS 0.020000
#endif

layout(set = 0, binding = 0) uniform sampler2DShadow u_shadow_atlas;
layout(set = 0, binding = 1) uniform sampler2D u_shadow_depth_linear;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

float sample_shadow(vec2 uv, float compare_depth) {
    return texture(u_shadow_atlas, vec3(ne_clamp_uv(uv), compare_depth));
}

float find_average_blocker(vec2 uv, float receiver_depth, float search_radius) {
    int samples = max(int(PCSS_BLOCKER_SAMPLES), 1);
    float blocker_sum = 0.0;
    float blocker_count = 0.0;
    for (int i = 0; i < samples; ++i) {
        vec2 offset = ne_vogel_disk_sample(i, samples) * search_radius;
        float d = texture(u_shadow_depth_linear, ne_clamp_uv(uv + offset)).r;
        if (d < receiver_depth - float(SHADOW_DEPTH_BIAS)) {
            blocker_sum += d;
            blocker_count += 1.0;
        }
    }
    if (blocker_count <= 0.0) {
        return -1.0;
    }
    return blocker_sum / blocker_count;
}

float pcss_visibility(vec2 uv, float receiver_depth) {
    vec2 texel = 1.0 / vec2(max(textureSize(u_shadow_depth_linear, 0).x, 1), max(textureSize(u_shadow_depth_linear, 0).y, 1));
    float base_radius = max(float(PCSS_LIGHT_RADIUS), 0.0001);
    float search_radius = base_radius * 32.0 * max(texel.x, texel.y);
    float blocker = find_average_blocker(uv, receiver_depth, search_radius);
    if (blocker < 0.0) {
        return 1.0;
    }
    float penumbra = clamp((receiver_depth - blocker) / max(blocker, NE_EPSILON), 0.0, 1.0);
    float filter_radius = base_radius * mix(8.0, 96.0, penumbra) * max(texel.x, texel.y);
    int samples = max(int(PCSS_FILTER_SAMPLES), 1);
    float visibility = 0.0;
    for (int i = 0; i < samples; ++i) {
        vec2 offset = ne_vogel_disk_sample(i, samples) * filter_radius;
        visibility += sample_shadow(uv + offset, receiver_depth - float(SHADOW_DEPTH_BIAS));
    }
    return visibility / float(samples);
}

void main() {
    float receiver_depth = texture(u_shadow_depth_linear, ne_clamp_uv(v_uv)).r;
    float visibility = pcss_visibility(v_uv, receiver_depth);
    o_color = vec4(vec3(visibility), 1.0);
}
