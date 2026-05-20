#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef BLOOM_RADIUS
#define BLOOM_RADIUS 1.000000
#endif
#ifndef BLOOM_MIP_INDEX
#define BLOOM_MIP_INDEX 0
#endif

layout(set = 0, binding = 0) uniform sampler2D u_source;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_source);
    float mip_radius = max(float(BLOOM_RADIUS), 0.5) * (1.0 + float(BLOOM_MIP_INDEX) * 0.25);
    vec3 c = ne_tent9(u_source, ne_clamp_uv(v_uv), texel, mip_radius);
    o_color = vec4(c, 1.0);
}
