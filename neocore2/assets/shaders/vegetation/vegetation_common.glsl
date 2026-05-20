#ifndef NEWENGINE_VEGETATION_COMMON_GLSL
#define NEWENGINE_VEGETATION_COMMON_GLSL

layout(std140, set = 2, binding = 0) uniform VegetationWindParams {
    vec4 world_time_strength;  // x=time, y=global strength, z=density scale, w=layer count
    vec4 layer0_dir_amp;       // xyz=direction, w=amplitude
    vec4 layer1_dir_amp;       // xyz=direction, w=amplitude
    vec4 gust_freq_phase;      // x=freq0, y=freq1, z=phase, w=reserved
} vegetation_wind;

layout(std430, set = 2, binding = 1) readonly buffer GrassInstanceBuffer {
    vec4 grass_pos_scale[];
};

layout(std430, set = 2, binding = 2) readonly buffer TreeImposterMetadataBuffer {
    vec4 imposter_pos_radius[];
};

layout(std430, set = 2, binding = 3) readonly buffer VegetationShadowEnvelopeBuffer {
    vec4 shadow_center_radius[];
};

vec3 vegetation_wind_offset(vec3 world_pos, float stiffness, float phase_bias) {
    float time = vegetation_wind.world_time_strength.x;
    float strength = vegetation_wind.world_time_strength.y;
    vec3 d0 = normalize(vegetation_wind.layer0_dir_amp.xyz + vec3(1e-4));
    vec3 d1 = normalize(vegetation_wind.layer1_dir_amp.xyz + vec3(1e-4));
    float wave0 = sin(dot(world_pos.xz, d0.xz) * 0.085 + time * vegetation_wind.gust_freq_phase.x + phase_bias);
    float wave1 = sin(dot(world_pos.xz, d1.xz) * 0.047 + time * vegetation_wind.gust_freq_phase.y + phase_bias * 1.37);
    float gust = wave0 * vegetation_wind.layer0_dir_amp.w + wave1 * vegetation_wind.layer1_dir_amp.w;
    return (d0 * gust * strength * clamp(1.0 - stiffness, 0.0, 1.0));
}

float vegetation_alpha_cutout(float alpha, float cutoff, float lod_fade) {
    float dither = fract(sin(gl_FragCoord.x * 12.9898 + gl_FragCoord.y * 78.233) * 43758.5453);
    return step(cutoff, alpha * lod_fade + dither * 0.035);
}

vec3 vegetation_wrap_lighting(vec3 normal_ws, vec3 light_dir_ws, vec3 sun_rgb, float wrap) {
    float ndl = dot(normalize(normal_ws), normalize(-light_dir_ws));
    float wrapped = clamp((ndl + wrap) / (1.0 + wrap), 0.0, 1.0);
    return sun_rgb * wrapped;
}

#endif
