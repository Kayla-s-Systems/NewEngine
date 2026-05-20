#version 450
#include "postfx_common.glsl"
void main() {
    float radius = 4.0;
    out_color = vec4(blur9(0, v_uv, radius), 1.0);
}
