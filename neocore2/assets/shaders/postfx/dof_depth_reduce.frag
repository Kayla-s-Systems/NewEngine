#version 450
#include "postfx_common.glsl"
void main() {
    vec2 t = texel_size(0);
    float d0 = sample_input_raw(0, v_uv).r;
    float d1 = sample_input_raw(0, v_uv + vec2(t.x, 0.0)).r;
    float d2 = sample_input_raw(0, v_uv + vec2(0.0, t.y)).r;
    float d3 = sample_input_raw(0, v_uv + t).r;
    float d = min(min(d0, d1), min(d2, d3));
    out_color = vec4(vec3(d), 1.0);
}
