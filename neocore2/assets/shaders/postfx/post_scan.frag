#version 450
#include "postfx_common.glsl"
void main() {
    vec3 c = sample_color(0, v_uv);
    float scan = sin(v_uv.y * 1080.0 * 3.14159) * 0.015;
    c += scan;
    c = apply_grade(c);
    out_color = vec4(max(c, vec3(0.0)), 1.0);
}
