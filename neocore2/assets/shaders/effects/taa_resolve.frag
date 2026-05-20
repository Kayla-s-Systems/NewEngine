#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef TAA_FEEDBACK
#define TAA_FEEDBACK 0.920000
#endif
#ifndef TAA_CLAMPING
#define TAA_CLAMPING 1.000000
#endif
#ifndef TAA_JITTER_SCALE
#define TAA_JITTER_SCALE 1.000000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_current_color;
layout(set = 0, binding = 1) uniform sampler2D u_history_color;
layout(set = 0, binding = 2) uniform sampler2D u_current_depth;
layout(set = 0, binding = 3) uniform sampler2D u_history_depth;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

float history_depth_weight(vec2 uv) {
    float cd = texture(u_current_depth, uv).r;
    float hd = texture(u_history_depth, uv).r;
    float diff = abs(cd - hd);
    return 1.0 - smoothstep(0.0005, 0.01 * max(float(TAA_JITTER_SCALE), 0.1), diff);
}

void main() {
    vec2 uv = ne_clamp_uv(v_uv);
    vec3 current = ne_safe_color(texture(u_current_color, uv).rgb);
    vec3 history = ne_bicubic_history_sample(u_history_color, uv);

    vec3 mn;
    vec3 mx;
    ne_neighborhood_minmax(u_current_color, uv, mn, mx);
    float clamp_strength = clamp(float(TAA_CLAMPING), 0.0, 1.0);
    history = mix(history, clamp(history, mn, mx), clamp_strength);

    float feedback = clamp(float(TAA_FEEDBACK), 0.0, 0.98) * history_depth_weight(uv);
    o_color = vec4(mix(current, history, feedback), 1.0);
}
