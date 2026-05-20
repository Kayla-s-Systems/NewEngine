#version 450
#include "postfx_common.glsl"
void main() {
    vec3 hdr = blur9(0, v_uv, 1.0);
    float threshold = pc.p[16];
    float knee = max(pc.p[17], 0.0001);
    vec3 bright = ne_soft_threshold(hdr, threshold, knee);
    out_color = vec4(bright, 1.0);
}
