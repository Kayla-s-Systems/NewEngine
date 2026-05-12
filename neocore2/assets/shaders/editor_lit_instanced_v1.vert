#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;

layout(location = 5) in vec4 i_model0;
layout(location = 6) in vec4 i_model1;
layout(location = 7) in vec4 i_model2;
layout(location = 8) in vec4 i_model3;
layout(location = 9) in vec4 i_mvp0;
layout(location = 10) in vec4 i_mvp1;
layout(location = 11) in vec4 i_mvp2;
layout(location = 12) in vec4 i_mvp3;
layout(location = 13) in vec4 i_base_color;
layout(location = 14) in vec4 i_uv_transform;
layout(location = 15) in vec4 i_material_params;
layout(location = 16) in vec4 i_emissive;

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
layout(location = 5) out vec4 v_material_params;
layout(location = 6) out vec4 v_emissive;

void main() {
    mat4 model = mat4(i_model0, i_model1, i_model2, i_model3);
    mat4 mvp = mat4(i_mvp0, i_mvp1, i_mvp2, i_mvp3);

    vec4 wpos4 = model * vec4(a_pos, 1.0);
    v_wpos = wpos4.xyz;
    v_wnrm = mat3(model) * a_nrm;
    v_base = i_base_color;
    v_uv = a_uv * i_uv_transform.xy + i_uv_transform.zw;
    v_light_clip = ubo.u_light_mvp * wpos4;
    v_material_params = i_material_params;
    v_emissive = i_emissive;
    gl_Position = mvp * vec4(a_pos, 1.0);
}
