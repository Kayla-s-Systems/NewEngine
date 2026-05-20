#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 1) in float v_lod_fade;
layout(set = 1, binding = 0) uniform sampler2D imposter_atlas;
layout(push_constant) uniform TreeImposterMaterialPush { vec4 light_wrap_alpha; } pc;
layout(location = 0) out vec4 out_color;
float vegetation_alpha_dither(float alpha, float fade) {
    float bayer = fract(sin(dot(gl_FragCoord.xy, vec2(41.0, 289.0))) * 951.1357);
    return step(bayer, alpha * fade);
}
void main() {
    vec4 c = texture(imposter_atlas, v_uv);
    if (vegetation_alpha_dither(c.a, v_lod_fade) < pc.light_wrap_alpha.y) discard;
    out_color = vec4(c.rgb * pc.light_wrap_alpha.xxx, 1.0);
}
