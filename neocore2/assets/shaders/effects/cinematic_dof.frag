#version 450
#include "common.glsl"

#ifndef DOF_SAMPLE_COUNT
#define DOF_SAMPLE_COUNT 24
#endif
#ifndef DOF_RADIUS_PX
#define DOF_RADIUS_PX 8.000000
#endif
#ifndef DOF_HIGHLIGHT_GAIN
#define DOF_HIGHLIGHT_GAIN 1.250000
#endif

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec2 texel = ne_rcp_texture_size(u_scene);
    vec3 center = texture(u_scene, ne_clamp_uv(v_uv)).rgb;
    vec3 sum = center;
    float wsum = 1.0;
    int count = clamp(int(DOF_SAMPLE_COUNT), 6, 48);
    for (int i = 0; i < count; ++i) {
        vec2 disk = ne_vogel_disk_sample(i, count);
        vec3 tap = texture(u_scene, ne_clamp_uv(v_uv + disk * texel * float(DOF_RADIUS_PX))).rgb;
        float highlight = smoothstep(1.0, 5.5, ne_luma(tap));
        float w = mix(1.0, 0.28, length(disk)) + highlight * float(DOF_HIGHLIGHT_GAIN);
        sum += tap * w;
        wsum += w;
    }
    vec3 bokeh = sum / max(wsum, NE_EPSILON);
    float radial = smoothstep(0.12, 0.74, length(v_uv - vec2(0.5)));
    o_color = vec4(mix(center, bokeh, radial), 1.0);
}
