#version 450

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;
layout(location = 3) in vec2 v_uv;
layout(location = 4) in vec4 v_light_clip;
layout(location = 5) in vec4 v_material_params;
layout(location = 6) in vec4 v_emissive;

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
    // x: normal_scale, y: roughness, z: metallic, w: occlusion_strength
    vec4 u_material_params;
    mat4 u_light_mvp;
    // x: enabled, y: base bias, z: shadow/contact strength, w: PCF softness radius
    vec4 u_shadow_params;
    // x: normal bias in shadow-depth units, y: cascade count, z/w: reserved for atlas/cascade metadata
    vec4 u_shadow_extra;
} ubo;
layout(set = 0, binding = 1) uniform texture2D u_base_tex;
layout(set = 0, binding = 2) uniform texture2D u_normal_tex;
layout(set = 0, binding = 3) uniform texture2D u_roughness_tex;
layout(set = 0, binding = 4) uniform texture2D u_shadow_tex;
layout(set = 0, binding = 5) uniform sampler u_material_sampler;

layout(location = 0) out vec4 o_color;

const float PI = 3.14159265359;
const float NE_EPS = 1.0e-5;

float saturate(float v) { return clamp(v, 0.0, 1.0); }
vec3 saturate3(vec3 v) { return clamp(v, vec3(0.0), vec3(1.0)); }
float ne_luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

vec3 safe_normalize(vec3 v, vec3 fallback) {
    float len2 = dot(v, v);
    return len2 > 1.0e-8 ? v * inversesqrt(len2) : fallback;
}

// Reference-derived material guard: runtime assets can arrive through texture dictionaries,
// but a missing/black layer should not collapse world geometry to an invisible plane.
vec3 material_texture_safe(vec3 sampled, vec3 tint, vec3 fallback) {
    float sampled_luma = ne_luma(sampled);
    float tint_luma = ne_luma(abs(tint));
    vec3 authored_fallback = mix(fallback, max(abs(tint), vec3(0.08)), saturate(tint_luma));
    return sampled_luma < 0.003 ? authored_fallback : sampled;
}

// Reoriented normal blend, adapted from the reference shader common layer.
vec3 reoriented_normal_blend(vec3 base_normal, vec3 detail_normal, float amount) {
    vec3 t = base_normal * vec3(2.0, 2.0, 2.0) + vec3(-1.0, -1.0, 0.0);
    vec3 u = detail_normal * vec3(-2.0, -2.0, 2.0) + vec3(1.0, 1.0, -1.0);
    vec3 blended = normalize(t * dot(t, u) - u * t.z);
    return normalize(mix(base_normal, blended, saturate(amount)));
}

float material_micro_shadow(float ndotl, float roughness, float occlusion) {
    float contact = smoothstep(0.02, 0.55, ndotl);
    float cavity = mix(0.55, 1.0, saturate(occlusion));
    return mix(contact, 1.0, roughness * 0.42) * cavity;
}

mat3 cotangent_frame(vec3 n, vec3 p, vec2 uv) {
    vec3 dp1 = dFdx(p);
    vec3 dp2 = dFdy(p);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, n);
    vec3 dp1perp = cross(n, dp1);
    vec3 t = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 b = dp2perp * duv1.y + dp1perp * duv2.y;
    float invmax = inversesqrt(max(max(dot(t, t), dot(b, b)), NE_EPS));
    return mat3(t * invmax, b * invmax, n);
}

float shadow_tap(vec2 uv, float current, float bias) {
    float closest = texture(sampler2D(u_shadow_tex, u_material_sampler), clamp(uv, vec2(0.001), vec2(0.999))).r;
    if (closest >= 0.9995) {
        return 1.0;
    }
    return (current - bias <= closest) ? 1.0 : 0.0;
}

float shadow_blocker_depth(vec2 uv, float current, float bias, vec2 texel) {
    float blocker_sum = 0.0;
    float blocker_count = 0.0;
    const vec2 taps[8] = vec2[8](
        vec2(-1.5, -0.5), vec2(-0.5, -1.5), vec2(0.5, -1.5), vec2(1.5, -0.5),
        vec2(1.5, 0.5), vec2(0.5, 1.5), vec2(-0.5, 1.5), vec2(-1.5, 0.5)
    );
    for (int i = 0; i < 8; ++i) {
        float d = texture(sampler2D(u_shadow_tex, u_material_sampler), clamp(uv + taps[i] * texel, vec2(0.001), vec2(0.999))).r;
        if (d < current - bias && d < 0.9995) {
            blocker_sum += d;
            blocker_count += 1.0;
        }
    }
    return blocker_count > 0.5 ? blocker_sum / blocker_count : -1.0;
}

float shadow_compare_quality(vec2 uv, float current, float bias) {
    float radius = clamp(ubo.u_shadow_params.w, 0.0, 1.25);
    if (radius <= 0.05) {
        return shadow_tap(uv, current, bias);
    }

    ivec2 sz = textureSize(sampler2D(u_shadow_tex, u_material_sampler), 0);
    vec2 texel = max(radius, 0.35) / vec2(max(sz.x, 1), max(sz.y, 1));

    // PCSS-lite: first estimate blockers, then expand the PCF kernel from receiver/blocker
    // separation. This keeps the common case cheap while avoiding the old fixed 9-tap cost.
    float blocker = shadow_blocker_depth(uv, current, bias, texel);
    float penumbra = blocker > 0.0 ? clamp((current - blocker) * 42.0 * radius, 0.55, 3.25) : 0.75;
    vec2 filter_texel = texel * penumbra;

    if (radius <= 0.75 && penumbra <= 1.25) {
        float lit4 = 0.0;
        lit4 += shadow_tap(uv + filter_texel * vec2(-0.5, -0.5), current, bias);
        lit4 += shadow_tap(uv + filter_texel * vec2( 0.5, -0.5), current, bias);
        lit4 += shadow_tap(uv + filter_texel * vec2(-0.5,  0.5), current, bias);
        lit4 += shadow_tap(uv + filter_texel * vec2( 0.5,  0.5), current, bias);
        return lit4 * 0.25;
    }

    float lit = 0.0;
    lit += shadow_tap(uv + filter_texel * vec2(-1.0, -1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + filter_texel * vec2( 0.0, -1.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + filter_texel * vec2( 1.0, -1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + filter_texel * vec2(-1.0,  0.0), current, bias) * 0.1250;
    lit += shadow_tap(uv,                                      current, bias) * 0.2500;
    lit += shadow_tap(uv + filter_texel * vec2( 1.0,  0.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + filter_texel * vec2(-1.0,  1.0), current, bias) * 0.0625;
    lit += shadow_tap(uv + filter_texel * vec2( 0.0,  1.0), current, bias) * 0.1250;
    lit += shadow_tap(uv + filter_texel * vec2( 1.0,  1.0), current, bias) * 0.0625;
    return lit;
}

float sample_shadow(vec4 light_clip, vec3 nrm, vec3 light_dir_to_scene) {
    if (ubo.u_shadow_params.x < 0.5 || light_clip.w <= 0.0) {
        return 1.0;
    }

    vec3 ndc = light_clip.xyz / light_clip.w;
    vec2 uv = ndc.xy * 0.5 + 0.5;
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }

    float current = ndc.z;
    float ndotl = max(dot(normalize(nrm), normalize(-light_dir_to_scene)), 0.0);
    float slope = 1.0 - ndotl;
    float receiver_bias = ubo.u_shadow_params.y * (1.0 + slope * 2.85);
    float normal_bias = clamp(ubo.u_shadow_extra.x, 0.0, 0.006) * slope;
    float bias = max(receiver_bias + normal_bias, 0.00005);
    float strength = clamp(ubo.u_shadow_params.z, 0.0, 0.78);

    float lit = shadow_compare_quality(uv, current, bias);
    float border = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
    float edge_fade = smoothstep(0.006, 0.055, border)
        * smoothstep(0.010, 0.080, ndc.z)
        * (1.0 - smoothstep(0.920, 0.992, ndc.z));
    float shadowed = mix(1.0 - strength, 1.0, lit);
    return mix(1.0, shadowed, edge_fade);
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
    float f = pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
    return F0 + (1.0 - F0) * f;
}

vec3 fresnel_schlick_roughness(float cosTheta, vec3 F0, float roughness) {
    float f = pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * f;
}

vec3 decode_normal_sample(vec3 packed) {
    vec2 xy = packed.xy * 2.0 - 1.0;
    float z = packed.z > 0.0039
        ? packed.z * 2.0 - 1.0
        : sqrt(max(1.0 - dot(xy, xy), 0.0));
    return normalize(vec3(xy, z));
}

vec3 apply_normal_map(vec3 N, vec3 wpos, vec2 uv, vec2 dx, vec2 dy, float normal_scale) {
    if (normal_scale <= 0.001) {
        return N;
    }
    vec3 packed_n = textureGrad(sampler2D(u_normal_tex, u_material_sampler), uv, dx, dy).xyz;
    vec3 map_n = decode_normal_sample(packed_n);
    mat3 tbn = cotangent_frame(N, wpos, uv);
    return normalize(tbn * vec3(map_n.xy * normal_scale, map_n.z));
}

vec3 pbr_direct(vec3 base, vec3 N, vec3 V, vec3 L, vec3 light_color, float intensity, float roughness, float metallic) {
    vec3 H = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotL <= 0.0 || NdotV <= 0.0 || intensity <= 0.0) {
        return vec3(0.0);
    }

    float LdotH = max(dot(L, H), 0.0);
    vec3 F0 = mix(vec3(0.04), base, metallic);
    float D = distribution_ggx(N, H, roughness);
    float G = geometry_smith(N, V, L, roughness);
    vec3 F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    vec3 specular = (D * G * F) / max(4.0 * NdotV * NdotL, 1.0e-4);
    specular = min(specular, vec3(8.0));

    // Disney/Burley-style rough diffuse. It gives rough terrain and bark a more
    // natural grazing response than pure Lambert while preserving energy balance.
    float fd90 = 0.5 + 2.0 * LdotH * LdotH * roughness;
    float light_scatter = 1.0 + (fd90 - 1.0) * pow(1.0 - NdotL, 5.0);
    float view_scatter = 1.0 + (fd90 - 1.0) * pow(1.0 - NdotV, 5.0);
    vec3 kD = (vec3(1.0) - F) * (1.0 - metallic);
    vec3 diffuse = kD * base * (light_scatter * view_scatter) / PI;

    return (diffuse + specular) * light_color * intensity * NdotL;
}

vec3 hemi_ambient(vec3 base, vec3 N, vec3 V, float roughness, float metallic, float occlusion) {
    vec3 ambient = max(ubo.u_ambient.rgb * ubo.u_ambient.a, vec3(0.0));
    float up = saturate(N.y * 0.5 + 0.5);
    vec3 sky = ambient * mix(vec3(0.84, 0.91, 1.08), vec3(1.12, 1.17, 1.24), up);
    vec3 ground = ambient * vec3(0.50, 0.47, 0.42);
    vec3 diffuse_ambient = mix(ground, sky, up) * base;

    vec3 F0 = mix(vec3(0.04), base, metallic);
    vec3 F = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, roughness);
    vec3 spec_ambient = sky * F * (1.0 - roughness) * 0.18;

    return (diffuse_ambient + spec_ambient) * occlusion;
}

float point_light_attenuation(float dist, float range) {
    float normalized = saturate(dist / max(range, 0.0001));
    float window = saturate(1.0 - normalized * normalized);
    return (window * window) / max(1.0 + dist * dist * 0.045, 1.0);
}

vec3 sky_dome_radiance(vec3 sampled, vec3 tint, vec3 view_dir, vec3 emissive_color) {
    vec3 to_sun = normalize(-ubo.u_dir_dir_intensity.xyz);
    float sun_elevation = to_sun.y;
    float day = smoothstep(-0.10, 0.22, sun_elevation);
    float horizon = pow(saturate(1.0 - abs(view_dir.y)), 1.65);

    vec3 night_zenith = vec3(0.008, 0.014, 0.035);
    vec3 day_zenith = vec3(0.16, 0.36, 0.78);
    vec3 zenith = mix(night_zenith, day_zenith, day);
    vec3 sunset = vec3(1.0, 0.50, 0.23);
    vec3 day_horizon = mix(vec3(0.54, 0.72, 0.96), sunset, smoothstep(-0.04, 0.18, sun_elevation) * (1.0 - smoothstep(0.16, 0.52, sun_elevation)));
    vec3 night_horizon = vec3(0.018, 0.022, 0.050);
    vec3 atmosphere = mix(zenith, mix(night_horizon, day_horizon, day), horizon);

    vec3 cloud_sample = material_texture_safe(sampled, tint, vec3(0.42, 0.46, 0.54));
    float density = saturate(max(max(cloud_sample.r, cloud_sample.g), cloud_sample.b) * 1.30);
    vec3 cloud_day = vec3(1.0, 0.965, 0.88);
    vec3 cloud_night = vec3(0.055, 0.065, 0.095);
    vec3 cloud_color = mix(cloud_night, cloud_day, day) * density * (0.20 + 0.65 * horizon);

    float sun_dot = saturate(dot(view_dir, to_sun));
    float disk = pow(sun_dot, 4096.0) * 3.8;
    float halo = pow(sun_dot, 64.0) * 0.18 + pow(sun_dot, 12.0) * 0.025;
    vec3 sun = ubo.u_dir_color.rgb * max(ubo.u_dir_dir_intensity.w, 0.0) * day * (disk + halo);

    return max(atmosphere + cloud_color + sun + emissive_color * 0.18, vec3(0.0));
}

void main() {
    vec2 stable_material_uv_dx = dFdx(v_uv);
    vec2 stable_material_uv_dy = dFdy(v_uv);
    vec3 N = safe_normalize(v_wnrm, vec3(0.0, 1.0, 0.0));
    vec4 material_params = v_material_params;
    vec4 emissive_color = v_emissive;

    float normal_scale = clamp(material_params.x, 0.0, 1.0);
    N = apply_normal_map(N, v_wpos, v_uv, stable_material_uv_dx, stable_material_uv_dy, normal_scale);

    vec4 texel = textureGrad(sampler2D(u_base_tex, u_material_sampler), v_uv, stable_material_uv_dx, stable_material_uv_dy);
    vec3 safe_texel = material_texture_safe(texel.rgb, v_base.rgb, vec3(0.68, 0.64, 0.56));
    vec3 base = max(saturate3(v_base.rgb * safe_texel), vec3(0.012));
    float roughness_sample_raw = textureGrad(sampler2D(u_roughness_tex, u_material_sampler), v_uv, stable_material_uv_dx, stable_material_uv_dy).r;
    float roughness_sample = roughness_sample_raw < 0.003 ? 1.0 : roughness_sample_raw;
    float roughness = clamp(material_params.y * max(roughness_sample, 0.08), 0.045, 1.0);
    float metallic = clamp(material_params.z, 0.0, 1.0);
    float occlusion = clamp(material_params.w, 0.06, 1.0);

    // `u_point_count_pad.yzw` carries the active camera world position.
    // It reuses std140 padding so the lit UBO size stays ABI-stable.
    vec3 camera_pos = ubo.u_point_count_pad.yzw;
    vec3 view_vec = camera_pos - v_wpos;
    float view_len2 = dot(view_vec, view_vec);
    vec3 V = view_len2 > 1.0e-6 ? view_vec * inversesqrt(view_len2) : vec3(0.0, 0.0, 1.0);

    float sky_mask = smoothstep(0.75, 1.45, max(max(emissive_color.r, emissive_color.g), emissive_color.b));
    if (sky_mask > 0.01) {
        vec3 sky_view_dir = safe_normalize(-V, safe_normalize(v_wpos, vec3(0.0, 1.0, 0.0)));
        vec3 sky = sky_dome_radiance(safe_texel, v_base.rgb, sky_view_dir, emissive_color.rgb);
        o_color = vec4(sky, 1.0);
        return;
    }

    vec3 color = hemi_ambient(base, N, V, roughness, metallic, occlusion);

    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float shadow = sample_shadow(v_light_clip, N, ubo.u_dir_dir_intensity.xyz);
    float sun_micro_shadow = material_micro_shadow(max(dot(N, Ld), 0.0), roughness, occlusion);
    color += shadow * sun_micro_shadow * pbr_direct(
        base,
        N,
        V,
        Ld,
        ubo.u_dir_color.rgb,
        ubo.u_dir_dir_intensity.w,
        roughness,
        metallic
    );

    int point_count = int(ubo.u_point_count_pad.x + 0.5);
    for (int i = 0; i < point_count && i < 4; ++i) {
        vec3 toL = ubo.u_point_pos_range[i].xyz - v_wpos;
        float dist = length(toL);
        float range = max(ubo.u_point_pos_range[i].w, 0.0001);
        vec3 L = toL / max(dist, 0.0001);
        float atten = point_light_attenuation(dist, range);
        float point_micro_shadow = material_micro_shadow(max(dot(N, L), 0.0), roughness, occlusion);
        color += atten * point_micro_shadow * pbr_direct(
            base,
            N,
            V,
            L,
            ubo.u_point_color_intensity[i].rgb,
            ubo.u_point_color_intensity[i].w,
            roughness,
            metallic
        );
    }

    color += emissive_color.rgb;

    o_color = vec4(max(color, vec3(0.0)), v_base.a * texel.a);
}
