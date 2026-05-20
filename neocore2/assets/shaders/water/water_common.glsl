#ifndef NEWENGINE_WATER_COMMON_GLSL
#define NEWENGINE_WATER_COMMON_GLSL

vec2 ne_water_scroll(vec2 uv, vec2 speed, float time_seconds, float scale) {
    return uv * scale + speed * time_seconds;
}

vec3 ne_unpack_normal_xy(vec2 packed_xy, float strength) {
    vec2 xy = packed_xy * 2.0 - 1.0;
    float z = sqrt(max(1.0 - dot(xy, xy), 0.0));
    return normalize(vec3(xy * strength, z));
}

float ne_fresnel_schlick(float cos_theta, float bias, float power) {
    float f = bias + (1.0 - bias) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), power);
    return clamp(f, 0.0, 1.0);
}

float ne_shoreline_foam(float depth_delta, float noise, float cutoff) {
    float edge = smoothstep(cutoff, 0.0, depth_delta);
    float breakup = smoothstep(0.25, 0.85, noise);
    return clamp(edge * breakup, 0.0, 1.0);
}

vec3 ne_apply_water_absorption(vec3 color, float depth_delta, vec3 shallow_tint, vec3 deep_tint) {
    float t = smoothstep(0.0, 8.0, max(depth_delta, 0.0));
    return mix(color * shallow_tint, color * deep_tint, t);
}

#endif
