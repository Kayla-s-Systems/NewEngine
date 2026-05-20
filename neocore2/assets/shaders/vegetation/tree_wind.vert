#version 450
layout(std140, set = 2, binding = 0) uniform VegetationWindParams { vec4 world_time_strength; vec4 layer0_dir_amp; vec4 layer1_dir_amp; vec4 gust_freq_phase; } vegetation_wind;
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_color;
layout(push_constant) uniform TreePush { mat4 model; mat4 view_proj; vec4 wind_stiffness_lod; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec3 v_normal_ws;
layout(location = 2) out vec3 v_world_pos;
layout(location = 3) out float v_lod_fade;
vec3 vegetation_wind_offset(vec3 world_pos, float stiffness, float phase_bias) {
    float time = vegetation_wind.world_time_strength.x;
    float strength = vegetation_wind.world_time_strength.y;
    vec3 d0 = normalize(vegetation_wind.layer0_dir_amp.xyz + vec3(0.0001));
    vec3 d1 = normalize(vegetation_wind.layer1_dir_amp.xyz + vec3(0.0001));
    float wave0 = sin(dot(world_pos.xz, d0.xz) * 0.085 + time * vegetation_wind.gust_freq_phase.x + phase_bias);
    float wave1 = sin(dot(world_pos.xz, d1.xz) * 0.047 + time * vegetation_wind.gust_freq_phase.y + phase_bias * 1.37);
    float gust = wave0 * vegetation_wind.layer0_dir_amp.w + wave1 * vegetation_wind.layer1_dir_amp.w;
    return d0 * gust * strength * clamp(1.0 - stiffness, 0.0, 1.0);
}
void main() {
    vec3 world = (pc.model * vec4(in_pos, 1.0)).xyz;
    float stiffness = clamp(in_color.a * pc.wind_stiffness_lod.x, 0.0, 1.0);
    world += vegetation_wind_offset(world, stiffness, in_color.r * 6.28318);
    v_uv = in_uv;
    v_normal_ws = normalize(mat3(pc.model) * in_normal);
    v_world_pos = world;
    v_lod_fade = pc.wind_stiffness_lod.y;
    gl_Position = pc.view_proj * vec4(world, 1.0);
}
