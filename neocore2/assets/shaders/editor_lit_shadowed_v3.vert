#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_emissive;
    vec4 u_ambient;
    vec4 u_dir_dir_intensity;
    vec4 u_dir_color;
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;
    vec4 u_uv_transform;
    vec4 u_material_params;
    mat4 u_light_mvp;
    vec4 u_shadow_params;
} ubo;

layout(location = 0) out vec3 v_wpos;
layout(location = 1) out vec3 v_wnrm;
layout(location = 2) out vec4 v_base;
layout(location = 3) out vec2 v_uv;
layout(location = 4) out vec4 v_light_clip;

void main() {
    vec4 wpos4 = ubo.u_model * vec4(a_pos, 1.0);
    v_wpos = wpos4.xyz;
    v_wnrm = mat3(ubo.u_model) * a_nrm;
    v_base = ubo.u_base_color;
    v_uv = a_uv * ubo.u_uv_transform.xy + ubo.u_uv_transform.zw;
    v_light_clip = ubo.u_light_mvp * wpos4;
    gl_Position = ubo.u_mvp * vec4(a_pos, 1.0);
}
