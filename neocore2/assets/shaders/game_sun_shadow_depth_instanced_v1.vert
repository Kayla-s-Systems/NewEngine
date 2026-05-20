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
    mat4 u_cascade_light_mvp[4];
    vec4 u_shadow_params;
    // x: normal bias in shadow-depth units, y: cascade count, z: tile resolution, w: max shadow distance
    vec4 u_shadow_extra;
    // per-cascade far split distances in world units from the camera
    vec4 u_shadow_splits;
} ubo;

layout(location = 0) out float v_depth;

void main() {
    mat4 mvp = mat4(i_mvp0, i_mvp1, i_mvp2, i_mvp3);
    vec4 clip = mvp * vec4(a_pos, 1.0);
    gl_Position = clip;
    float ndc_z = clip.z / max(clip.w, 1.0e-6);
    v_depth = clamp(ndc_z, 0.0, 1.0);
}
