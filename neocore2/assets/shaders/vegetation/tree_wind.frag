#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_normal_ws;
layout(location = 2) in vec3 v_world_pos;
layout(location = 3) in float v_lod_fade;
layout(set = 1, binding = 0) uniform sampler2D tree_albedo_alpha;
layout(set = 1, binding = 1) uniform sampler2D tree_normal;
layout(push_constant) uniform TreeMaterialPush { vec4 sun_dir_intensity; vec4 sun_rgb_ambient; vec4 material_params; } pc;
layout(location = 0) out vec4 out_color;
float vegetation_alpha_dither(float alpha, float fade) {
    float bayer = fract(sin(dot(gl_FragCoord.xy, vec2(41.0, 289.0))) * 951.1357);
    return step(bayer, alpha * fade);
}
vec3 vegetation_wrap_lighting(vec3 normal_ws, vec3 light_dir_ws, vec3 sun_rgb, float wrap) {
    float ndl = dot(normalize(normal_ws), normalize(-light_dir_ws));
    float wrapped = clamp((ndl + wrap) / (1.0 + wrap), 0.0, 1.0);
    return sun_rgb * wrapped;
}
void main() {
    vec4 albedo = texture(tree_albedo_alpha, v_uv);
    if (vegetation_alpha_dither(albedo.a, v_lod_fade) < pc.material_params.x) discard;
    vec3 normal = normalize(v_normal_ws + (texture(tree_normal, v_uv).xyz * 2.0 - 1.0) * 0.35);
    vec3 lit = vegetation_wrap_lighting(normal, pc.sun_dir_intensity.xyz, pc.sun_rgb_ambient.rgb * pc.sun_dir_intensity.w, pc.material_params.y);
    vec3 ambient = pc.sun_rgb_ambient.www * vec3(0.55, 0.58, 0.50);
    out_color = vec4(albedo.rgb * (ambient + lit), 1.0);
}
