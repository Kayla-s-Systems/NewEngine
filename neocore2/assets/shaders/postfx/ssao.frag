#version 450
#include "postfx_common.glsl"
void main() {
    vec2 uv = v_uv;
    vec2 texel = texel_size(1);
    float center = sample_input_raw(1, uv).r;
    vec3 n = normalize(sample_color(2, uv) * 2.0 - 1.0);
    float occ = 0.0;
    float wsum = 0.0;
    const int count = 16;
    for (int i = 0; i < count; ++i) {
        vec2 disk = ne_vogel_disk_sample(i, count);
        float r = length(disk);
        float w = 1.0 - r * 0.65;
        float sample_depth = sample_input_raw(1, uv + disk * texel * 4.0).r;
        occ += smoothstep(0.012, 0.145, center - sample_depth) * w;
        wsum += w;
    }
    float facing = clamp(n.z * 0.5 + 0.5, 0.25, 1.0);
    float ao = 1.0 - clamp(occ / max(wsum, 0.000001), 0.0, 1.0) * 0.78 * facing;
    out_color = vec4(vec3(ao), 1.0);
}
