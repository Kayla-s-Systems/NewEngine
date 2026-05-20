#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

layout(location = 0) in vec3 te_position;
layout(location = 1) in vec3 te_normal;
layout(location = 2) in vec2 te_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec3 n = normalize(te_normal) * 0.5 + 0.5;
    float grid = abs(fract(te_uv.x * 16.0) - 0.5) + abs(fract(te_uv.y * 16.0) - 0.5);
    float wire_hint = smoothstep(0.0, 0.06, grid);
    o_color = vec4(mix(vec3(0.08, 0.11, 0.15), n, wire_hint), 1.0);
}
