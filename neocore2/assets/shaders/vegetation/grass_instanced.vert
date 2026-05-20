#version 450
layout(std140, set = 2, binding = 0) uniform VegetationWindParams {
    vec4 world_time_strength;
    vec4 layer0_dir_amp;
    vec4 layer1_dir_amp;
    vec4 gust_freq_phase;
} vegetation_wind;
layout(std430, set = 2, binding = 1) readonly buffer GrassInstanceBuffer { vec4 grass_pos_scale[]; };
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;
layout(push_constant) uniform GrassDrawPush { mat4 view_proj; vec4 camera_pos_lod; } pc;
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
    vec4 inst = grass_pos_scale[gl_InstanceIndex];
    vec3 world = inst.xyz + in_pos * inst.w;
    float stiffness = clamp(1.0 - in_uv.y, 0.0, 1.0);
    world += vegetation_wind_offset(world, stiffness, float(gl_InstanceIndex) * 0.071);
    v_uv = in_uv;
    v_normal_ws = normalize(in_normal + vec3(0.0, 0.35, 0.0));
    v_world_pos = world;
    float distance_m = distance(pc.camera_pos_lod.xyz, world);
    v_lod_fade = smoothstep(pc.camera_pos_lod.w + 8.0, pc.camera_pos_lod.w, distance_m);
    gl_Position = pc.view_proj * vec4(world, 1.0);
}
