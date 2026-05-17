#version 450

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;
layout(location = 3) in vec2 v_uv;
layout(location = 4) in vec4 v_light_clip;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_emissive;
    vec4 u_ambient;
    vec4 u_dir_dir_intensity;
    vec4 u_dir_color;
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;
    vec4 u_uv_transform;
    // x: biome_patch_scale, y: biome_blend_softness, z: terrain_roughness, w: occlusion_strength
    vec4 u_material_params;
    mat4 u_light_mvp;
    // x: enabled, y: base bias, z: shadow/contact strength, w: PCF softness radius
    vec4 u_shadow_params;
} ubo;

// Terrain pipeline intentionally reuses the lit bind group layout:
// binding 1 = forest/grass albedo, binding 2 = sand albedo, binding 3 = rock/moss albedo.
layout(set = 0, binding = 1) uniform texture2D u_forest_tex;
layout(set = 0, binding = 2) uniform texture2D u_sand_tex;
layout(set = 0, binding = 3) uniform texture2D u_rock_tex;
layout(set = 0, binding = 4) uniform texture2D u_shadow_tex;
layout(set = 0, binding = 5) uniform sampler u_material_sampler;

layout(location = 0) out vec4 o_color;

const float PI = 3.14159265359;
const float NE_EPS = 1.0e-5;

float saturate(float v) { return clamp(v, 0.0, 1.0); }
vec3 saturate3(vec3 v) { return clamp(v, vec3(0.0), vec3(1.0)); }

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 hash22(vec2 p) {
    return vec2(hash12(p + vec2(17.17, 3.31)), hash12(p + vec2(5.97, 41.13)));
}

float value_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash12(i + vec2(0.0, 0.0));
    float b = hash12(i + vec2(1.0, 0.0));
    float c = hash12(i + vec2(0.0, 1.0));
    float d = hash12(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float a = 0.5;
    float s = 0.0;
    float n = 0.0;
    for (int i = 0; i < 5; ++i) {
        s += value_noise(p) * a;
        n += a;
        p = mat2(1.62, 1.17, -1.17, 1.62) * p + vec2(17.13, -9.41);
        a *= 0.5;
    }
    return s / max(n, 1.0e-4);
}

vec3 terrain_weights(vec3 world_pos, vec3 world_normal) {
    float patch_scale = max(ubo.u_material_params.x, 0.0025);
    float softness = clamp(ubo.u_material_params.y, 0.01, 0.45);
    vec3 n = normalize(world_normal);
    vec2 p = world_pos.xz * patch_scale;

    float continent = fbm(p * 0.42 + vec2(-17.0, 9.0));
    float meadow = fbm(p * 0.95 + vec2(23.0, -41.0));
    float eroded_path = fbm(vec2(p.x * 0.72 + p.y * 0.18, p.y * 1.85 - p.x * 0.08) + vec2(11.0, 71.0));
    float gravel = fbm(p * 2.65 + vec2(-8.0, 25.0));

    float slope = 1.0 - clamp(n.y, 0.0, 1.0);
    float slope_flatness = smoothstep(0.78, 0.99, n.y);
    float lowland = 1.0 - smoothstep(-0.05, 1.25, world_pos.y);

    float sand_path = smoothstep(0.55 - softness, 0.78 + softness, eroded_path)
        * slope_flatness
        * mix(0.55, 1.0, lowland);
    float sand_basin = smoothstep(0.30 - softness, 0.62 + softness, 1.0 - continent)
        * slope_flatness
        * 0.68;
    float sand = max(sand_path, sand_basin);

    float rock_slope = smoothstep(0.23 - softness, 0.58 + softness, slope);
    float rock_outcrop = smoothstep(0.76 - softness, 0.94 + softness, gravel)
        * smoothstep(0.18, 1.35, world_pos.y)
        * 0.72;
    float rock = max(rock_slope, rock_outcrop);

    float forest_clearings = smoothstep(0.18 - softness, 0.64 + softness, meadow);
    sand *= 1.0 - rock * 0.58;
    rock *= 1.0 - sand * 0.35;
    float forest = max(0.0, mix(0.72, 1.0, forest_clearings) - sand - rock);

    vec3 w = vec3(forest, sand, rock);
    w = max(w, vec3(0.0));
    return w / max(w.x + w.y + w.z, 1.0e-4);
}

vec3 sample_forest_layer(vec2 uv, vec2 dx, vec2 dy, vec2 world_xz) {
    float layer_seed = 1.0;
    float scale = 1.00;
    vec2 cell = floor(world_xz * 0.037 * scale + layer_seed);
    vec2 jitter_a = (hash22(cell + layer_seed) - 0.5) * 3.7;
    vec2 jitter_b = (hash22(cell + vec2(19.0, -23.0) + layer_seed) - 0.5) * 7.1;
    float blend = smoothstep(0.22, 0.78, fbm(world_xz * 0.031 * scale + vec2(layer_seed * 11.0, -layer_seed * 7.0)));
    vec3 a = textureGrad(sampler2D(u_forest_tex, u_material_sampler), uv + jitter_a, dx, dy).rgb;
    vec3 b = textureGrad(sampler2D(u_forest_tex, u_material_sampler), uv * 1.73 + jitter_b, dx * 1.73, dy * 1.73).rgb;
    float micro = fbm(world_xz * 0.19 * scale + vec2(layer_seed * 3.0, layer_seed * 5.0));
    return mix(a, b, blend * 0.42) * mix(0.92, 1.08, micro);
}

vec3 sample_sand_layer(vec2 uv, vec2 dx, vec2 dy, vec2 world_xz) {
    float layer_seed = 2.0;
    float scale = 0.82;
    vec2 cell = floor(world_xz * 0.037 * scale + layer_seed);
    vec2 jitter_a = (hash22(cell + layer_seed) - 0.5) * 3.7;
    vec2 jitter_b = (hash22(cell + vec2(19.0, -23.0) + layer_seed) - 0.5) * 7.1;
    float blend = smoothstep(0.22, 0.78, fbm(world_xz * 0.031 * scale + vec2(layer_seed * 11.0, -layer_seed * 7.0)));
    vec3 a = textureGrad(sampler2D(u_sand_tex, u_material_sampler), uv + jitter_a, dx, dy).rgb;
    vec3 b = textureGrad(sampler2D(u_sand_tex, u_material_sampler), uv * 1.73 + jitter_b, dx * 1.73, dy * 1.73).rgb;
    float micro = fbm(world_xz * 0.19 * scale + vec2(layer_seed * 3.0, layer_seed * 5.0));
    return mix(a, b, blend * 0.42) * mix(0.92, 1.08, micro);
}

vec3 sample_rock_layer(vec2 uv, vec2 dx, vec2 dy, vec2 world_xz) {
    float layer_seed = 3.0;
    float scale = 1.35;
    vec2 cell = floor(world_xz * 0.037 * scale + layer_seed);
    vec2 jitter_a = (hash22(cell + layer_seed) - 0.5) * 3.7;
    vec2 jitter_b = (hash22(cell + vec2(19.0, -23.0) + layer_seed) - 0.5) * 7.1;
    float blend = smoothstep(0.22, 0.78, fbm(world_xz * 0.031 * scale + vec2(layer_seed * 11.0, -layer_seed * 7.0)));
    vec3 a = textureGrad(sampler2D(u_rock_tex, u_material_sampler), uv + jitter_a, dx, dy).rgb;
    vec3 b = textureGrad(sampler2D(u_rock_tex, u_material_sampler), uv * 1.73 + jitter_b, dx * 1.73, dy * 1.73).rgb;
    float micro = fbm(world_xz * 0.19 * scale + vec2(layer_seed * 3.0, layer_seed * 5.0));
    return mix(a, b, blend * 0.42) * mix(0.92, 1.08, micro);
}

float shadow_tap(vec2 uv, float current, float bias) {
    float closest = texture(sampler2D(u_shadow_tex, u_material_sampler), clamp(uv, vec2(0.001), vec2(0.999))).r;
    if (closest >= 0.9995) {
        return 1.0;
    }
    return (current - bias <= closest) ? 1.0 : 0.0;
}

float shadow_compare_quality(vec2 uv, float current, float bias) {
    ivec2 sz = textureSize(sampler2D(u_shadow_tex, u_material_sampler), 0);
    vec2 texel = clamp(ubo.u_shadow_params.w, 0.25, 1.75) / vec2(max(sz.x, 1), max(sz.y, 1));
    float lit = 0.0;
    lit += shadow_tap(uv + texel * vec2(-1.0, -1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + texel * vec2( 0.0, -1.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + texel * vec2( 1.0, -1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + texel * vec2(-1.0,  0.0), current, bias) * 0.1250;
    lit += shadow_tap(uv,                              current, bias) * 0.2500;
    lit += shadow_tap(uv + texel * vec2( 1.0,  0.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + texel * vec2(-1.0,  1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + texel * vec2( 0.0,  1.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + texel * vec2( 1.0,  1.0), current, bias) * 0.0625;
    return lit;
}

float sample_shadow(vec4 light_clip, vec3 nrm, vec3 light_dir_to_scene) {
    if (ubo.u_shadow_params.x < 0.5 || light_clip.w <= 0.0) {
        return 1.0;
    }

    vec3 ndc = light_clip.xyz / light_clip.w;
    vec2 uv = ndc.xy * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999) {
        return 1.0;
    }

    float current = clamp(ndc.z, 0.0, 1.0);
    float ndotl = max(dot(normalize(nrm), normalize(-light_dir_to_scene)), 0.0);
    float slope = 1.0 - ndotl;
    float bias = max(ubo.u_shadow_params.y * (1.0 + slope * 2.85) + 0.00018 * slope, 0.00005);
    float strength = clamp(ubo.u_shadow_params.z, 0.0, 0.78);

    float lit = shadow_compare_quality(uv, current, bias);
    return mix(1.0 - strength, 1.0, lit);
}

float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return a2 / max(PI * denom * denom, NE_EPS);
}

float geometry_schlick_ggx(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) * 0.125;
    return NdotV / max(NdotV * (1.0 - k) + k, NE_EPS);
}

float geometry_smith(vec3 N, vec3 V, vec3 L, float roughness) {
    float ggx1 = geometry_schlick_ggx(max(dot(N, V), 0.0), roughness);
    float ggx2 = geometry_schlick_ggx(max(dot(N, L), 0.0), roughness);
    return ggx1 * ggx2;
}

vec3 fresnel_schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 pbr_direct(vec3 base, vec3 N, vec3 V, vec3 L, vec3 light_color, float intensity, float roughness) {
    vec3 H = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotL <= 0.0 || NdotV <= 0.0 || intensity <= 0.0) {
        return vec3(0.0);
    }

    float LdotH = max(dot(L, H), 0.0);
    vec3 F0 = vec3(0.04);
    float D = distribution_ggx(N, H, roughness);
    float G = geometry_smith(N, V, L, roughness);
    vec3 F = fresnel_schlick(max(dot(H, V), 0.0), F0);
    vec3 specular = min((D * G * F) / max(4.0 * NdotV * NdotL, 1.0e-4), vec3(5.0));

    float fd90 = 0.5 + 2.0 * LdotH * LdotH * roughness;
    float light_scatter = 1.0 + (fd90 - 1.0) * pow(1.0 - NdotL, 5.0);
    float view_scatter = 1.0 + (fd90 - 1.0) * pow(1.0 - NdotV, 5.0);
    vec3 diffuse = (vec3(1.0) - F) * base * (light_scatter * view_scatter) / PI;
    return (diffuse + specular) * light_color * intensity * NdotL;
}

vec3 terrain_ambient(vec3 base, vec3 N, vec3 V, float roughness, float occlusion) {
    vec3 ambient = max(ubo.u_ambient.rgb * ubo.u_ambient.a, vec3(0.0));
    float up = saturate(N.y * 0.5 + 0.5);
    vec3 sky = ambient * mix(vec3(0.86, 0.93, 1.10), vec3(1.12, 1.17, 1.23), up);
    vec3 ground = ambient * vec3(0.46, 0.43, 0.38);
    vec3 diffuse_ambient = mix(ground, sky, up) * base;

    vec3 F = fresnel_schlick(max(dot(N, V), 0.0), vec3(0.04));
    vec3 grazing = sky * F * (1.0 - roughness) * 0.10;
    return (diffuse_ambient + grazing) * occlusion;
}

float point_light_attenuation(float dist, float range) {
    float normalized = saturate(dist / max(range, 0.0001));
    float window = saturate(1.0 - normalized * normalized);
    return (window * window) / max(1.0 + dist * dist * 0.045, 1.0);
}

void main() {
    vec2 dx = dFdx(v_uv);
    vec2 dy = dFdy(v_uv);
    vec3 N = normalize(v_wnrm);
    vec3 camera_pos = ubo.u_point_count_pad.yzw;
    vec3 view_vec = camera_pos - v_wpos;
    float view_len2 = dot(view_vec, view_vec);
    vec3 V = view_len2 > 1.0e-6 ? view_vec * inversesqrt(view_len2) : vec3(0.0, 0.0, 1.0);

    vec3 w = terrain_weights(v_wpos, N);
    vec3 forest = sample_forest_layer(v_uv, dx, dy, v_wpos.xz);
    vec3 sand = sample_sand_layer(v_uv * 0.82, dx * 0.82, dy * 0.82, v_wpos.xz);
    vec3 rock = sample_rock_layer(v_uv * 1.35, dx * 1.35, dy * 1.35, v_wpos.xz);

    vec3 base = saturate3((forest * w.x + sand * w.y + rock * w.z) * v_base.rgb);
    float macro_variation = fbm(v_wpos.xz * 0.012 + vec2(5.0, -3.0));
    float slope = 1.0 - saturate(N.y);
    base *= mix(0.84, 1.10, macro_variation);
    base *= mix(vec3(1.0), vec3(0.88, 0.90, 0.94), slope * w.z * 0.45);

    float roughness = clamp(ubo.u_material_params.z + slope * 0.08, 0.20, 1.0);
    float procedural_occlusion = mix(0.92, 1.0, fbm(v_wpos.xz * 0.09 + vec2(91.0, -14.0)));
    float occlusion = clamp(ubo.u_material_params.w * procedural_occlusion, 0.0, 1.0);

    vec3 color = terrain_ambient(base, N, V, roughness, occlusion);

    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float shadow = sample_shadow(v_light_clip, N, ubo.u_dir_dir_intensity.xyz);
    color += shadow * pbr_direct(base, N, V, Ld, ubo.u_dir_color.rgb, ubo.u_dir_dir_intensity.w, roughness);

    int point_count = int(ubo.u_point_count_pad.x + 0.5);
    for (int i = 0; i < point_count && i < 4; ++i) {
        vec3 toL = ubo.u_point_pos_range[i].xyz - v_wpos;
        float dist = length(toL);
        float range = max(ubo.u_point_pos_range[i].w, 0.0001);
        vec3 L = toL / max(dist, 0.0001);
        float atten = point_light_attenuation(dist, range);
        color += atten * pbr_direct(base, N, V, L, ubo.u_point_color_intensity[i].rgb, ubo.u_point_color_intensity[i].w, roughness);
    }

    color += ubo.u_emissive.rgb;
    o_color = vec4(max(color, vec3(0.0)), v_base.a);
}
