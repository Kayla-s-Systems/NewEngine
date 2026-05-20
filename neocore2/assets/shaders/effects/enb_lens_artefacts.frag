#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef LENS_GHOST_STRENGTH
#define LENS_GHOST_STRENGTH 0.650000
#endif
#ifndef LENS_STREAK_STRENGTH
#define LENS_STREAK_STRENGTH 0.420000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

float ne_ghost(vec2 uv, vec2 center, float scale, float radius) {
    vec2 p = center + (center - uv) * scale;
    float l = ne_luma(texture(u_scene, ne_clamp_uv(p)).rgb);
    float d = length(v_uv - p);
    return smoothstep(radius, 0.0, d) * smoothstep(0.75, 4.0, l);
}

void main() {
    vec3 base = texture(u_scene, ne_clamp_uv(v_uv)).rgb;
    vec2 center = vec2(0.5);
    vec2 dir = normalize(v_uv - center + vec2(0.0001));
    float ghosts = 0.0;
    ghosts += ne_ghost(v_uv, center, 0.34, 0.030) * 0.80;
    ghosts += ne_ghost(v_uv, center, 0.82, 0.055) * 0.48;
    ghosts += ne_ghost(v_uv, center, 1.42, 0.078) * 0.30;
    float streak = 0.0;
    vec2 texel = ne_rcp_texture_size(u_scene);
    for (int i = -4; i <= 4; ++i) {
        vec3 tap = texture(u_scene, ne_clamp_uv(v_uv + dir.yx * texel * float(i) * 14.0)).rgb;
        streak += smoothstep(1.0, 7.0, ne_luma(tap)) / (1.0 + abs(float(i)));
    }
    vec3 tint = vec3(1.0, 0.82, 0.55);
    o_color = vec4(base + tint * ghosts * float(LENS_GHOST_STRENGTH) + tint * streak * float(LENS_STREAK_STRENGTH) * 0.08, 1.0);
}
