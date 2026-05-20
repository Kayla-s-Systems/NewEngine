#version 450
#include "postfx_common.glsl"
void main() {
    vec2 t = texel_size(0);
    vec3 c = sample_color(0, v_uv);
    vec3 n = sample_color(0, v_uv + vec2(0.0, -t.y));
    vec3 s = sample_color(0, v_uv + vec2(0.0,  t.y));
    vec3 e = sample_color(0, v_uv + vec2( t.x, 0.0));
    vec3 w = sample_color(0, v_uv + vec2(-t.x, 0.0));
    float edge = length(n + s - 2.0 * c) + length(e + w - 2.0 * c);
    vec3 aa = (c * 4.0 + n + s + e + w) / 8.0;
    out_color = vec4(mix(c, aa, clamp(edge * pc.p[21], 0.0, 1.0)), 1.0);
}
