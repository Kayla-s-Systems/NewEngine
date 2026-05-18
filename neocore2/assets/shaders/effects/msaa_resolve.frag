#version 450
#include "common.glsl"

#ifndef MSAA_SAMPLES
#define MSAA_SAMPLES 4
#endif
#ifndef MSAA_GAMMA_CORRECT
#define MSAA_GAMMA_CORRECT 1
#endif

layout(set = 0, binding = 0) uniform sampler2DMS u_msaa_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

vec3 decode_sample(vec3 c) {
#if MSAA_GAMMA_CORRECT
    return pow(max(c, vec3(0.0)), vec3(2.2));
#else
    return c;
#endif
}

vec3 encode_color(vec3 c) {
#if MSAA_GAMMA_CORRECT
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
#else
    return c;
#endif
}

void main() {
    ivec2 size_px = textureSize(u_msaa_scene);
    ivec2 px = clamp(ivec2(v_uv * vec2(size_px)), ivec2(0), max(size_px - ivec2(1), ivec2(0)));
    vec4 accum = vec4(0.0);
    int samples = clamp(int(MSAA_SAMPLES), 1, 8);
    for (int i = 0; i < samples; ++i) {
        vec4 s = texelFetch(u_msaa_scene, px, i);
        accum.rgb += decode_sample(s.rgb);
        accum.a += s.a;
    }
    accum /= float(samples);
    o_color = vec4(encode_color(accum.rgb), accum.a);
}
