#version 450

layout (location = 0) in vec3 in_pos;
layout (location = 1) in vec3 in_nrm;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_ambient;            // rgb + intensity
    vec4 u_dir_dir_intensity;  // xyz direction (incoming rays) + intensity
    vec4 u_dir_color;          // rgb
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;    // x = count
} ubo;

layout (location = 0) out vec3 v_pos_ws;
layout (location = 1) out vec3 v_nrm_ws;

void main() {
    vec4 pos_ws = ubo.u_model * vec4(in_pos, 1.0);
    v_pos_ws = pos_ws.xyz;
    // Best-effort normal transform: mat3(model) + renormalize.
    v_nrm_ws = normalize(mat3(ubo.u_model) * in_nrm);
    gl_Position = ubo.u_mvp * vec4(in_pos, 1.0);
}
