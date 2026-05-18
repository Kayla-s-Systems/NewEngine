#version 450
#include "common.glsl"

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;

layout(location = 0) out vec3 v_position;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;

void main() {
    v_position = a_position;
    v_normal = normalize(a_normal);
    v_uv = a_uv;
    gl_Position = vec4(a_position, 1.0);
}
