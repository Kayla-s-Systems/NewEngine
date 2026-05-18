// NewEngine Vulkan effect shader common helpers.
// Inspired by modern multipass post-processing stacks: thresholded bloom,
// separable tent/gaussian sampling, luminance-safe blending, temporal clamp,
// depth-aware PCSS helpers. This file intentionally contains no #version line.

#ifndef NE_EFFECT_COMMON_GLSL
#define NE_EFFECT_COMMON_GLSL 1
#endif

#ifndef NE_PI
#define NE_PI 3.14159265358979323846
#endif

#ifndef NE_EPSILON
#define NE_EPSILON 0.000001
#endif

vec2 ne_rcp_texture_size(sampler2D tex) {
    ivec2 sz = textureSize(tex, 0);
    return 1.0 / vec2(max(sz.x, 1), max(sz.y, 1));
}

vec2 ne_clamp_uv(vec2 uv) {
    return clamp(uv, vec2(0.0), vec2(1.0));
}

vec3 ne_safe_color(vec3 c) {
    return clamp(c, vec3(0.0), vec3(65504.0));
}

float ne_luma(vec3 c) {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

vec3 ne_saturate_bloom(vec3 bloom, float saturation) {
    float grey = ne_luma(bloom);
    return mix(vec3(grey), bloom, clamp(saturation, 0.0, 2.0));
}

vec3 ne_soft_threshold(vec3 color, float threshold, float knee) {
    color = ne_safe_color(color);
    float brightness = max(max(color.r, color.g), color.b);
    float soft = max(knee, NE_EPSILON);
    float rq = clamp((brightness - threshold + soft) / (2.0 * soft), 0.0, 1.0);
    rq = rq * rq * soft;
    float hard = max(brightness - threshold, 0.0);
    float contribution = max(hard, rq) / max(brightness, NE_EPSILON);
    return color * contribution;
}

vec3 ne_tent9(sampler2D tex, vec2 uv, vec2 texel, float radius) {
    radius = max(radius, 0.5);
    vec3 sum = texture(tex, uv).rgb * 4.0;
    sum += texture(tex, uv + vec2( texel.x, 0.0) * radius).rgb * 2.0;
    sum += texture(tex, uv + vec2(-texel.x, 0.0) * radius).rgb * 2.0;
    sum += texture(tex, uv + vec2(0.0,  texel.y) * radius).rgb * 2.0;
    sum += texture(tex, uv + vec2(0.0, -texel.y) * radius).rgb * 2.0;
    sum += texture(tex, uv + vec2( texel.x,  texel.y) * radius).rgb;
    sum += texture(tex, uv + vec2(-texel.x,  texel.y) * radius).rgb;
    sum += texture(tex, uv + vec2( texel.x, -texel.y) * radius).rgb;
    sum += texture(tex, uv + vec2(-texel.x, -texel.y) * radius).rgb;
    return ne_safe_color(sum / 16.0);
}

vec3 ne_bicubic_history_sample(sampler2D tex, vec2 uv) {
    // Catmull-Rom approximation from four bilinear taps. Good enough for TAA history
    // without requiring textureGather support on every target profile.
    ivec2 size_px = textureSize(tex, 0);
    vec2 size = vec2(max(size_px.x, 1), max(size_px.y, 1));
    vec2 inv_size = 1.0 / size;
    vec2 sample_pos = uv * size - 0.5;
    vec2 f = fract(sample_pos);
    vec2 base = (floor(sample_pos) + 0.5) * inv_size;

    vec3 c00 = texture(tex, base + vec2(-1.0, -1.0) * inv_size).rgb;
    vec3 c10 = texture(tex, base + vec2( 0.0, -1.0) * inv_size).rgb;
    vec3 c20 = texture(tex, base + vec2( 1.0, -1.0) * inv_size).rgb;
    vec3 c30 = texture(tex, base + vec2( 2.0, -1.0) * inv_size).rgb;
    vec3 c01 = texture(tex, base + vec2(-1.0,  0.0) * inv_size).rgb;
    vec3 c11 = texture(tex, base + vec2( 0.0,  0.0) * inv_size).rgb;
    vec3 c21 = texture(tex, base + vec2( 1.0,  0.0) * inv_size).rgb;
    vec3 c31 = texture(tex, base + vec2( 2.0,  0.0) * inv_size).rgb;
    vec3 c02 = texture(tex, base + vec2(-1.0,  1.0) * inv_size).rgb;
    vec3 c12 = texture(tex, base + vec2( 0.0,  1.0) * inv_size).rgb;
    vec3 c22 = texture(tex, base + vec2( 1.0,  1.0) * inv_size).rgb;
    vec3 c32 = texture(tex, base + vec2( 2.0,  1.0) * inv_size).rgb;
    vec3 c03 = texture(tex, base + vec2(-1.0,  2.0) * inv_size).rgb;
    vec3 c13 = texture(tex, base + vec2( 0.0,  2.0) * inv_size).rgb;
    vec3 c23 = texture(tex, base + vec2( 1.0,  2.0) * inv_size).rgb;
    vec3 c33 = texture(tex, base + vec2( 2.0,  2.0) * inv_size).rgb;

    vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    vec2 w1 = vec2(1.0) + f * f * (-2.5 + 1.5 * f);
    vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    vec2 w3 = f * f * (-0.5 + 0.5 * f);

    vec3 row0 = c00 * w0.x + c10 * w1.x + c20 * w2.x + c30 * w3.x;
    vec3 row1 = c01 * w0.x + c11 * w1.x + c21 * w2.x + c31 * w3.x;
    vec3 row2 = c02 * w0.x + c12 * w1.x + c22 * w2.x + c32 * w3.x;
    vec3 row3 = c03 * w0.x + c13 * w1.x + c23 * w2.x + c33 * w3.x;
    return ne_safe_color(row0 * w0.y + row1 * w1.y + row2 * w2.y + row3 * w3.y);
}

void ne_neighborhood_minmax(sampler2D tex, vec2 uv, out vec3 mn, out vec3 mx) {
    vec2 t = ne_rcp_texture_size(tex);
    vec3 center = texture(tex, uv).rgb;
    mn = center;
    mx = center;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            vec3 c = texture(tex, uv + vec2(x, y) * t).rgb;
            mn = min(mn, c);
            mx = max(mx, c);
        }
    }
}

float ne_vogel_disk_angle(int i, int n) {
    return float(i) * 2.39996323;
}

vec2 ne_vogel_disk_sample(int i, int n) {
    float r = sqrt((float(i) + 0.5) / max(float(n), 1.0));
    float a = ne_vogel_disk_angle(i, n);
    return vec2(cos(a), sin(a)) * r;
}
