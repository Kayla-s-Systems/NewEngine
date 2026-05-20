#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef BLOOM_INTENSITY
#define BLOOM_INTENSITY 1.000000
#endif
#ifndef BLOOM_SATURATION
#define BLOOM_SATURATION 1.000000
#endif
#ifndef BLOOM_BLEND_MODE
#define BLOOM_BLEND_MODE 0
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(set = 0, binding = 1) uniform sampler2D u_bloom;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

vec3 blend_add(vec3 scene, vec3 bloom) { return scene + bloom; }
vec3 blend_screen(vec3 scene, vec3 bloom) { return 1.0 - (1.0 - clamp(scene, 0.0, 1.0)) * (1.0 - clamp(bloom, 0.0, 1.0)); }
vec3 blend_max(vec3 scene, vec3 bloom) { return max(scene, bloom); }

void main() {
    vec3 scene = ne_safe_color(texture(u_scene, ne_clamp_uv(v_uv)).rgb);
    vec3 bloom = ne_safe_color(texture(u_bloom, ne_clamp_uv(v_uv)).rgb);
    bloom = ne_saturate_bloom(bloom, float(BLOOM_SATURATION)) * max(float(BLOOM_INTENSITY), 0.0);

#if BLOOM_BLEND_MODE == 1
    vec3 color = blend_screen(scene, bloom);
#elif BLOOM_BLEND_MODE == 2
    vec3 color = blend_max(scene, bloom);
#else
    vec3 color = blend_add(scene, bloom);
#endif
    o_color = vec4(ne_safe_color(color), 1.0);
}
