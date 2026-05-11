#version 450

layout (location = 0) in vec3 in_pos;
layout (location = 1) in vec4 in_col;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_ambient;
    vec4 u_dir_dir_intensity;
    vec4 u_dir_color;
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;
} ubo;

layout (location = 0) out vec4 v_col;

void main() {
    v_col = in_col;
    gl_Position = ubo.u_mvp * vec4(in_pos, 1.0);
}
