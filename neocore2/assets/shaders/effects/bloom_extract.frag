#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef BLOOM_THRESHOLD
#define BLOOM_THRESHOLD 0.850000
#endif
#ifndef BLOOM_KNEE
#define BLOOM_KNEE 0.350000
#endif
#ifndef BLOOM_RADIUS
#define BLOOM_RADIUS 1.000000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene_hdr;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_scene_hdr);
    vec3 hdr = ne_tent9(u_scene_hdr, ne_clamp_uv(v_uv), texel, max(float(BLOOM_RADIUS), 0.5));
    vec3 bright = ne_soft_threshold(hdr, float(BLOOM_THRESHOLD), float(BLOOM_KNEE));
    o_color = vec4(bright, 1.0);
}
