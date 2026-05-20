#version 450
#include "postfx_common.glsl"
void main() {
    vec3 sharp = sample_color(0, v_uv);
    float depth = sample_input_raw(1, v_uv).r;
    float focus = 0.5;
    float blur_amount = clamp(abs(depth - focus) * 2.5, 0.0, 1.0);
    vec3 blurred = blur9(0, v_uv, mix(1.0, 6.0, blur_amount));
    out_color = vec4(mix(sharp, blurred, blur_amount), 1.0);
}
