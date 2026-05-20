#version 450
#include "postfx_common.glsl"
void main() {
    out_color = vec4(blur9(0, v_uv, 1.0), 1.0);
}
