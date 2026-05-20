#version 450
#include "postfx_common.glsl"
float ghost_tap(vec2 uv, vec2 center, float scale, float radius) {
    vec2 p = center + (center - uv) * scale;
    float l = sample_luma(sample_color(0, p));
    float d = length(v_uv - p);
    return smoothstep(radius, 0.0, d) * smoothstep(0.75, 4.0, l);
}
void main() {
    vec3 base = sample_color(0, v_uv);
    vec2 center = vec2(0.5);
    vec2 dir = normalize(v_uv - center + vec2(0.0001));
    float ghosts = 0.0;
    ghosts += ghost_tap(v_uv, center, 0.34, 0.030) * 0.80;
    ghosts += ghost_tap(v_uv, center, 0.82, 0.055) * 0.48;
    ghosts += ghost_tap(v_uv, center, 1.42, 0.078) * 0.30;
    float streak = 0.0;
    vec2 texel = texel_size(0);
    for (int i = -4; i <= 4; ++i) {
        vec3 tap = sample_color(0, v_uv + dir.yx * texel * float(i) * 14.0);
        streak += smoothstep(1.0, 7.0, sample_luma(tap)) / (1.0 + abs(float(i)));
    }
    vec3 tint = vec3(1.0, 0.82, 0.55);
    out_color = vec4(base + tint * ghosts * max(pc.p[12], 0.0) + tint * streak * max(pc.p[13], 0.0) * 0.08, 1.0);
}
