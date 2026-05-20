#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_normal_ws;
layout(location = 2) in float v_branch_lod;
layout(set = 1, binding = 0) uniform sampler2D spm_albedo_alpha;
layout(push_constant) uniform SpmTreeMaterialPush { vec4 sun_dir_intensity; vec4 sun_rgb_ambient; vec4 material; } pc;
layout(location = 0) out vec4 out_color;
float vegetation_alpha_cutout(float alpha, float cutoff, float lod_fade) {
    float dither = fract(sin(gl_FragCoord.x * 12.9898 + gl_FragCoord.y * 78.233) * 43758.5453);
    return step(cutoff, alpha * lod_fade + dither * 0.035);
}
vec3 vegetation_wrap_lighting(vec3 normal_ws, vec3 light_dir_ws, vec3 sun_rgb, float wrap) {
    float ndl = dot(normalize(normal_ws), normalize(-light_dir_ws));
    float wrapped = clamp((ndl + wrap) / (1.0 + wrap), 0.0, 1.0);
    return sun_rgb * wrapped;
}
void main() {
    vec4 albedo = texture(spm_albedo_alpha, v_uv);
    if (vegetation_alpha_cutout(albedo.a, pc.material.x, 1.0 - v_branch_lod * 0.15) < 0.5) discard;
    vec3 lit = vegetation_wrap_lighting(v_normal_ws, pc.sun_dir_intensity.xyz, pc.sun_rgb_ambient.rgb * pc.sun_dir_intensity.w, pc.material.y);
    vec3 ambient = vec3(pc.sun_rgb_ambient.w);
    out_color = vec4(albedo.rgb * (ambient + lit), 1.0);
}
