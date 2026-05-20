#version 450
layout(std140, set = 2, binding = 0) uniform VegetationWindParams { vec4 world_time_strength; vec4 layer0_dir_amp; vec4 layer1_dir_amp; vec4 gust_freq_phase; } vegetation_wind;
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_branch_weight;
layout(push_constant) uniform SpmTreePush { mat4 model; mat4 view_proj; vec4 spm_params; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec3 v_normal_ws;
layout(location = 2) out float v_branch_lod;
vec3 vegetation_wind_offset(vec3 world_pos, float stiffness, float phase_bias) {
    float time = vegetation_wind.world_time_strength.x;
    float strength = vegetation_wind.world_time_strength.y;
    vec3 d0 = normalize(vegetation_wind.layer0_dir_amp.xyz + vec3(0.0001));
    float wave0 = sin(dot(world_pos.xz, d0.xz) * 0.085 + time * vegetation_wind.gust_freq_phase.x + phase_bias);
    return d0 * wave0 * vegetation_wind.layer0_dir_amp.w * strength * clamp(1.0 - stiffness, 0.0, 1.0);
}
void main() {
    vec3 world = (pc.model * vec4(in_pos, 1.0)).xyz;
    world += vegetation_wind_offset(world, in_branch_weight.x, in_branch_weight.y * 6.28318) * pc.spm_params.x;
    v_uv = in_uv;
    v_normal_ws = normalize(mat3(pc.model) * in_normal);
    v_branch_lod = in_branch_weight.z;
    gl_Position = pc.view_proj * vec4(world, 1.0);
}
