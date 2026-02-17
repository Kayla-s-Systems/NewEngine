#version 450
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec4 a_col;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
} u;

layout (location = 0) out vec4 v_col;

void main() {
    v_col = a_col;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
