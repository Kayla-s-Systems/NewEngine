#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef GRASS_WIND_STRENGTH
#define GRASS_WIND_STRENGTH 0.420000
#endif
#ifndef GRASS_SHADOW_FADE
#define GRASS_SHADOW_FADE 0.760000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_albedo;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec3 albedo = texture(u_albedo, ne_clamp_uv(v_uv)).rgb;
    float blade = smoothstep(0.12, 0.92, v_uv.y);
    float wind = sin(v_uv.x * 37.0 + v_uv.y * 11.0) * 0.5 + 0.5;
    vec3 root = vec3(0.075, 0.105, 0.040);
    vec3 tip = vec3(0.295, 0.470, 0.150);
    vec3 grass = mix(root, tip, blade) * mix(0.82, 1.18, wind * float(GRASS_WIND_STRENGTH));
    grass *= mix(float(GRASS_SHADOW_FADE), 1.0, blade);
    o_color = vec4(mix(grass, albedo, 0.35), 1.0);
}
