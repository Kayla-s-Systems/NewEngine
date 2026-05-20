#version 450
#include "postfx_common.glsl"
void main() {
    vec3 hdr = sample_color(0, v_uv);
    vec3 bloom = sample_color(1, v_uv) * pc.p[18];
    float ao = sample_color(2, v_uv).r;
    vec3 color = (hdr + bloom) * mix(1.0, ao, 0.55);
    color *= exp2(pc.p[0]);
    color = max(color - pc.p[2], vec3(0.0));
    if (pc.p[3] < 0.5) {
        color = aces_approx(color);
    } else if (pc.p[3] < 1.5) {
        color = color / (color + vec3(1.0));
    }
    color = pow(max(apply_grade(color), vec3(0.0)), vec3(1.0 / max(pc.p[1], 0.0001)));
    float vignette = smoothstep(0.95, 0.25, length(v_uv - 0.5)) * pc.p[27];
    color *= mix(1.0 - pc.p[27], 1.0, vignette);
    out_color = vec4(color, 1.0);
}
