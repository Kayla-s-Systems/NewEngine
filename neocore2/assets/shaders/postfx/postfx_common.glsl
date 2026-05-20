layout(set = 0, binding = 0) uniform sampler2D uPostFxInput0;
layout(set = 0, binding = 1) uniform sampler2D uPostFxInput1;
layout(set = 0, binding = 2) uniform sampler2D uPostFxInput2;
layout(set = 0, binding = 3) uniform sampler2D uPostFxInput3;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;

layout(push_constant) uniform PostFxPush {
    float p[32];
} pc;

vec4 sample_input_raw(int idx, vec2 uv) {
    uv = clamp(uv, vec2(0.0), vec2(1.0));
    if (idx == 1) return texture(uPostFxInput1, uv);
    if (idx == 2) return texture(uPostFxInput2, uv);
    if (idx == 3) return texture(uPostFxInput3, uv);
    return texture(uPostFxInput0, uv);
}

ivec2 input_size(int idx) {
    if (idx == 1) return textureSize(uPostFxInput1, 0);
    if (idx == 2) return textureSize(uPostFxInput2, 0);
    if (idx == 3) return textureSize(uPostFxInput3, 0);
    return textureSize(uPostFxInput0, 0);
}

vec3 sample_color(int idx, vec2 uv) {
    return sample_input_raw(idx, uv).rgb;
}

float sample_luma(vec3 c) {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

vec2 texel_size(int idx) {
    vec2 s = vec2(input_size(idx));
    return 1.0 / max(s, vec2(1.0));
}

vec3 aces_approx(vec3 x) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

vec3 apply_grade(vec3 c) {
    float saturation = pc.p[24];
    float contrast = pc.p[25];
    float temperature = pc.p[26];
    float l = sample_luma(c);
    c = mix(vec3(l), c, saturation);
    c = (c - 0.5) * max(contrast, 0.0) + 0.5;
    c.r += temperature * 0.025;
    c.b -= temperature * 0.025;
    return max(c, vec3(0.0));
}

vec3 blur9(int idx, vec2 uv, float radius_px) {
    vec2 t = texel_size(idx) * radius_px;
    vec3 c = sample_color(idx, uv) * 0.20;
    c += sample_color(idx, uv + vec2( t.x, 0.0)) * 0.12;
    c += sample_color(idx, uv + vec2(-t.x, 0.0)) * 0.12;
    c += sample_color(idx, uv + vec2(0.0,  t.y)) * 0.12;
    c += sample_color(idx, uv + vec2(0.0, -t.y)) * 0.12;
    c += sample_color(idx, uv + vec2( t.x,  t.y)) * 0.08;
    c += sample_color(idx, uv + vec2(-t.x,  t.y)) * 0.08;
    c += sample_color(idx, uv + vec2( t.x, -t.y)) * 0.08;
    c += sample_color(idx, uv + vec2(-t.x, -t.y)) * 0.08;
    return c;
}

vec3 ne_safe_color(vec3 c) {
    return clamp(c, vec3(0.0), vec3(65504.0));
}

vec3 ne_soft_threshold(vec3 color, float threshold, float knee) {
    color = ne_safe_color(color);
    float brightness = max(max(color.r, color.g), color.b);
    float soft = max(knee, 0.000001);
    float rq = clamp((brightness - threshold + soft) / (2.0 * soft), 0.0, 1.0);
    rq = rq * rq * soft;
    float hard = max(brightness - threshold, 0.0);
    float contribution = max(hard, rq) / max(brightness, 0.000001);
    return color * contribution;
}

vec2 ne_vogel_disk_sample(int i, int n) {
    float r = sqrt((float(i) + 0.5) / max(float(n), 1.0));
    float a = float(i) * 2.39996323;
    return vec2(cos(a), sin(a)) * r;
}

void ne_neighborhood_minmax(int idx, vec2 uv, out vec3 mn, out vec3 mx) {
    vec2 t = texel_size(idx);
    vec3 center = sample_color(idx, uv);
    mn = center;
    mx = center;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            vec3 c = sample_color(idx, uv + vec2(x, y) * t);
            mn = min(mn, c);
            mx = max(mx, c);
        }
    }
}

vec3 ne_bicubic_history_sample(int idx, vec2 uv) {
    ivec2 size_px = input_size(idx);
    vec2 size = vec2(max(size_px.x, 1), max(size_px.y, 1));
    vec2 inv_size = 1.0 / size;
    vec2 sample_pos = uv * size - 0.5;
    vec2 f = fract(sample_pos);
    vec2 base = (floor(sample_pos) + 0.5) * inv_size;
    vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    vec2 w1 = vec2(1.0) + f * f * (-2.5 + 1.5 * f);
    vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    vec2 w3 = f * f * (-0.5 + 0.5 * f);
    vec3 row0 = sample_color(idx, base + vec2(-1.0, -1.0) * inv_size) * w0.x
              + sample_color(idx, base + vec2( 0.0, -1.0) * inv_size) * w1.x
              + sample_color(idx, base + vec2( 1.0, -1.0) * inv_size) * w2.x
              + sample_color(idx, base + vec2( 2.0, -1.0) * inv_size) * w3.x;
    vec3 row1 = sample_color(idx, base + vec2(-1.0,  0.0) * inv_size) * w0.x
              + sample_color(idx, base + vec2( 0.0,  0.0) * inv_size) * w1.x
              + sample_color(idx, base + vec2( 1.0,  0.0) * inv_size) * w2.x
              + sample_color(idx, base + vec2( 2.0,  0.0) * inv_size) * w3.x;
    vec3 row2 = sample_color(idx, base + vec2(-1.0,  1.0) * inv_size) * w0.x
              + sample_color(idx, base + vec2( 0.0,  1.0) * inv_size) * w1.x
              + sample_color(idx, base + vec2( 1.0,  1.0) * inv_size) * w2.x
              + sample_color(idx, base + vec2( 2.0,  1.0) * inv_size) * w3.x;
    vec3 row3 = sample_color(idx, base + vec2(-1.0,  2.0) * inv_size) * w0.x
              + sample_color(idx, base + vec2( 0.0,  2.0) * inv_size) * w1.x
              + sample_color(idx, base + vec2( 1.0,  2.0) * inv_size) * w2.x
              + sample_color(idx, base + vec2( 2.0,  2.0) * inv_size) * w3.x;
    return ne_safe_color(row0 * w0.y + row1 * w1.y + row2 * w2.y + row3 * w3.y);
}
