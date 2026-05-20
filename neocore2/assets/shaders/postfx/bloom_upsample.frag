#version 450
#include "postfx_common.glsl"
void main() {
    vec3 bloom = blur9(0, v_uv, max(pc.p[19], 1.0));
    out_color = vec4(bloom * pc.p[18], 1.0);
}
