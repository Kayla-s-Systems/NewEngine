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

layout(location = 0) out float v_depth;

void main() {
    vec4 clip = ubo.u_mvp * vec4(a_pos, 1.0);
    gl_Position = clip;
    float ndc_z = clip.z / max(clip.w, 1.0e-6);
    v_depth = clamp(ndc_z, 0.0, 1.0);
}
