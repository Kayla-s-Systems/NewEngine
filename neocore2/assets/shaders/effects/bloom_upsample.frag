#version 450
#include "common.glsl"

#ifndef BLOOM_RADIUS
#define BLOOM_RADIUS 1.000000
#endif
#ifndef BLOOM_BLEND
#define BLOOM_BLEND 0.650000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_low_mip;
layout(set = 0, binding = 1) uniform sampler2D u_high_mip;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_low_mip);
    vec3 low = ne_tent9(u_low_mip, ne_clamp_uv(v_uv), texel, max(float(BLOOM_RADIUS), 0.5));
    vec3 high = texture(u_high_mip, ne_clamp_uv(v_uv)).rgb;
    o_color = vec4(ne_safe_color(high + low * clamp(float(BLOOM_BLEND), 0.0, 1.0)), 1.0);
}
