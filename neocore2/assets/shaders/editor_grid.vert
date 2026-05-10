#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec4 a_color;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
} ubo;

layout(location = 0) out vec4 v_color;

void main() {
    v_color = a_color;
    gl_Position = ubo.u_mvp * vec4(a_pos, 1.0);
}
