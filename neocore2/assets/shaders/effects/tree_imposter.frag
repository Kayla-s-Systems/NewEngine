#version 450
#include "common.glsl"

#ifndef TREE_IMPOSTER_ALPHA_CUTOFF
#define TREE_IMPOSTER_ALPHA_CUTOFF 0.330000
#endif
#ifndef TREE_IMPOSTER_LIGHT_WRAP
#define TREE_IMPOSTER_LIGHT_WRAP 0.380000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_imposter_atlas;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec4 tex = texture(u_imposter_atlas, ne_clamp_uv(v_uv));
    if (tex.a < float(TREE_IMPOSTER_ALPHA_CUTOFF)) {
        discard;
    }
    float canopy = smoothstep(0.06, 0.88, v_uv.y);
    float wrap = mix(1.0 - float(TREE_IMPOSTER_LIGHT_WRAP), 1.0 + float(TREE_IMPOSTER_LIGHT_WRAP) * 0.35, canopy);
    vec3 leaf_tint = mix(vec3(0.16, 0.20, 0.08), vec3(0.36, 0.48, 0.18), canopy);
    vec3 color = mix(leaf_tint, tex.rgb, 0.58) * wrap;
    o_color = vec4(color, tex.a);
}
