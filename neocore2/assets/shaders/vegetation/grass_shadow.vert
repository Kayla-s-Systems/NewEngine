#version 450
layout(std140, set = 2, binding = 0) uniform VegetationWindParams { vec4 world_time_strength; vec4 layer0_dir_amp; vec4 layer1_dir_amp; vec4 gust_freq_phase; } vegetation_wind;
layout(std430, set = 2, binding = 1) readonly buffer GrassInstanceBuffer { vec4 grass_pos_scale[]; };
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(push_constant) uniform GrassShadowPush { mat4 light_view_proj; vec4 params; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out float v_fade;
vec3 vegetation_wind_offset(vec3 world_pos, float stiffness, float phase_bias) {
    float time = vegetation_wind.world_time_strength.x;
    float strength = vegetation_wind.world_time_strength.y;
    vec3 d0 = normalize(vegetation_wind.layer0_dir_amp.xyz + vec3(0.0001));
    float wave0 = sin(dot(world_pos.xz, d0.xz) * 0.085 + time * vegetation_wind.gust_freq_phase.x + phase_bias);
    return d0 * wave0 * vegetation_wind.layer0_dir_amp.w * strength * clamp(1.0 - stiffness, 0.0, 1.0);
}
void main() {
    vec4 inst = grass_pos_scale[gl_InstanceIndex];
    vec3 world = inst.xyz + in_pos * inst.w;
    world += vegetation_wind_offset(world, clamp(1.0 - in_uv.y, 0.0, 1.0), float(gl_InstanceIndex) * 0.071) * pc.params.x;
    v_uv = in_uv;
    v_fade = pc.params.y;
    gl_Position = pc.light_view_proj * vec4(world, 1.0);
}
