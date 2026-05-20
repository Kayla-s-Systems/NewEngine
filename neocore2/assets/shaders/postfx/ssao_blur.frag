#version 450
#include "postfx_common.glsl"
void main() {
    float ao = blur9(0, v_uv, 1.25).r;
    out_color = vec4(vec3(clamp(ao, 0.0, 1.0)), 1.0);
}
