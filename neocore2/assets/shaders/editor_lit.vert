#version 450
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_nrm;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
    vec4 u_base_color;
} u;

layout (location = 0) out vec3 v_nrm;

void main() {
    v_nrm = a_nrm;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
