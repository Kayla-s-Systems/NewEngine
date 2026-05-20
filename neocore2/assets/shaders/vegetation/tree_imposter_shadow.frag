#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 1) in float v_fade;
layout(set = 1, binding = 0) uniform sampler2D imposter_atlas;
layout(push_constant) uniform TreeImposterShadowMaterialPush { vec4 params; } pc;
float vegetation_alpha_cutout(float alpha, float cutoff, float lod_fade) {
    float dither = fract(sin(gl_FragCoord.x * 12.9898 + gl_FragCoord.y * 78.233) * 43758.5453);
    return step(cutoff, alpha * lod_fade + dither * 0.035);
}
void main() {
    float alpha = texture(imposter_atlas, v_uv).a;
    if (vegetation_alpha_cutout(alpha, pc.params.x, v_fade) < 0.5) discard;
}
