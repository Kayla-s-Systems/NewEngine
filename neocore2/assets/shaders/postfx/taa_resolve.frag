#version 450
#include "postfx_common.glsl"
float history_depth_weight(vec2 uv) {
    float cd = sample_input_raw(3, uv).r;
    float hd = sample_input_raw(1, uv).r;
    float diff = abs(cd - hd);
    return 1.0 - smoothstep(0.0005, 0.01, diff);
}
void main() {
    vec2 uv = clamp(v_uv, vec2(0.0), vec2(1.0));
    vec3 current = ne_safe_color(sample_color(0, uv));
    vec3 history = ne_bicubic_history_sample(2, uv);
    vec3 mn;
    vec3 mx;
    ne_neighborhood_minmax(0, uv, mn, mx);
    history = clamp(history, mn, mx);
    float feedback = clamp(pc.p[31], 0.0, 0.98) * history_depth_weight(uv);
    out_color = vec4(mix(current, history, feedback), 1.0);
}
