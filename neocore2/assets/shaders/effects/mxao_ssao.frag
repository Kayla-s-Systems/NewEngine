#version 450
#include "common.glsl"

#ifndef MXAO_SAMPLE_COUNT
#define MXAO_SAMPLE_COUNT 16
#endif
#ifndef MXAO_RADIUS_PX
#define MXAO_RADIUS_PX 4.000000
#endif
#ifndef MXAO_INTENSITY
#define MXAO_INTENSITY 0.780000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene_depth_or_luma;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

float ne_scene_depth_proxy(vec2 uv) {
    vec3 c = texture(u_scene_depth_or_luma, ne_clamp_uv(uv)).rgb;
    return dot(c, vec3(0.333333));
}

void main() {
    vec2 texel = ne_rcp_texture_size(u_scene_depth_or_luma);
    float center = ne_scene_depth_proxy(v_uv);
    float occ = 0.0;
    float wsum = 0.0;
    int count = clamp(int(MXAO_SAMPLE_COUNT), 4, 32);
    for (int i = 0; i < count; ++i) {
        vec2 disk = ne_vogel_disk_sample(i, count);
        float r = length(disk);
        float w = 1.0 - r * 0.65;
        float sample_depth = ne_scene_depth_proxy(v_uv + disk * texel * float(MXAO_RADIUS_PX));
        occ += smoothstep(0.012, 0.145, center - sample_depth) * w;
        wsum += w;
    }
    float ao = 1.0 - clamp(occ / max(wsum, NE_EPSILON), 0.0, 1.0) * float(MXAO_INTENSITY);
    o_color = vec4(vec3(ao), 1.0);
}
