#version 450
#include "postfx_common.glsl"
void main() {
    vec3 hdr = sample_color(0, v_uv);
    hdr *= exp2(pc.p[0]);
    vec3 color = aces_approx(hdr);
    color = apply_grade(color);
    color = pow(max(color, vec3(0.0)), vec3(1.0 / max(pc.p[1], 0.0001)));
    out_color = vec4(color, 1.0);
}
