#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef WATER_FOAM_STRENGTH
#define WATER_FOAM_STRENGTH 0.360000
#endif
#ifndef WATER_ROUGHNESS
#define WATER_ROUGHNESS 0.180000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene_reflection;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

float ne_wave(vec2 p) {
    return sin(p.x * 41.0 + p.y * 17.0) * 0.5 + sin(p.x * 13.0 - p.y * 29.0) * 0.5;
}

void main() {
    vec2 uv = v_uv;
    float wave = ne_wave(uv);
    vec2 distortion = vec2(dFdx(wave), dFdy(wave)) * (0.015 + float(WATER_ROUGHNESS) * 0.035);
    vec3 reflection = texture(u_scene_reflection, ne_clamp_uv(uv + distortion)).rgb;
    vec3 deep = vec3(0.018, 0.075, 0.110);
    vec3 shallow = vec3(0.065, 0.185, 0.205);
    float fresnel = pow(1.0 - abs(uv.y * 2.0 - 1.0), 2.0);
    float foam = smoothstep(0.58, 0.90, abs(wave)) * float(WATER_FOAM_STRENGTH);
    vec3 water = mix(deep, shallow, fresnel) + reflection * mix(0.18, 0.55, fresnel);
    water = mix(water, vec3(0.82, 0.95, 0.92), foam);
    o_color = vec4(water, 1.0);
}
