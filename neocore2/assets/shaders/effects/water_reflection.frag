#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef WATER_REFLECTION_BLUR
#define WATER_REFLECTION_BLUR 2.000000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_scene);
    vec2 uv = vec2(v_uv.x, 1.0 - v_uv.y);
    vec3 sum = texture(u_scene, ne_clamp_uv(uv)).rgb * 4.0;
    sum += texture(u_scene, ne_clamp_uv(uv + vec2( texel.x, 0.0) * float(WATER_REFLECTION_BLUR))).rgb;
    sum += texture(u_scene, ne_clamp_uv(uv + vec2(-texel.x, 0.0) * float(WATER_REFLECTION_BLUR))).rgb;
    sum += texture(u_scene, ne_clamp_uv(uv + vec2(0.0,  texel.y) * float(WATER_REFLECTION_BLUR))).rgb;
    sum += texture(u_scene, ne_clamp_uv(uv + vec2(0.0, -texel.y) * float(WATER_REFLECTION_BLUR))).rgb;
    o_color = vec4(sum / 8.0, 1.0);
}
