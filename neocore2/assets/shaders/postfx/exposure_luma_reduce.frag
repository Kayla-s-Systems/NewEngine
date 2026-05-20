#version 450
#include "postfx_common.glsl"
void main() {
    vec3 c = sample_color(0, v_uv);
    float l = max(sample_luma(c), 0.0001);
    out_color = vec4(vec3(log2(l + 1.0)), 1.0);
}
