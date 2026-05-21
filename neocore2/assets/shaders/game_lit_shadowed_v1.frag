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
    // x: normal_scale, y: roughness, z: metallic, w: occlusion_strength
    vec4 u_material_params;
    mat4 u_light_mvp;
    mat4 u_cascade_light_mvp[4];
    // x: enabled, y: base bias, z: shadow/contact strength, w: PCF softness radius
    vec4 u_shadow_params;
    // x: normal bias in shadow-depth units, y: cascade count, z: tile resolution, w: max shadow distance
    vec4 u_shadow_extra;
    // per-cascade far split distances in world units from the camera
    vec4 u_shadow_splits;
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
    if (uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0) {
        return 1.0;
    }
    float closest = texture(sampler2D(u_shadow_tex, u_material_sampler), uv).r;
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
        vec2 tap_uv = uv + taps[i] * texel;
        if (tap_uv.x <= 0.0 || tap_uv.x >= 1.0 || tap_uv.y <= 0.0 || tap_uv.y >= 1.0) {
            continue;
        }
        float d = texture(sampler2D(u_shadow_tex, u_material_sampler), tap_uv).r;
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

vec4 shadow_clip_for_receiver(vec3 wpos, out vec2 tile_offset, out vec2 tile_scale, out float cascade_fade) {
    int cascade_count = int(clamp(floor(ubo.u_shadow_extra.y + 0.5), 1.0, 4.0));
    if (cascade_count <= 1) {
        tile_offset = vec2(0.0);
        tile_scale = vec2(1.0);
        cascade_fade = 1.0;
        return ubo.u_light_mvp * vec4(wpos, 1.0);
    }

    float view_distance = distance(wpos, ubo.u_point_count_pad.yzw);
    int cascade_index = 0;
    if (cascade_count > 1 && view_distance > ubo.u_shadow_splits.x) { cascade_index = 1; }
    if (cascade_count > 2 && view_distance > ubo.u_shadow_splits.y) { cascade_index = 2; }
    if (cascade_count > 3 && view_distance > ubo.u_shadow_splits.z) { cascade_index = 3; }

    float split_far = cascade_index == 0 ? ubo.u_shadow_splits.x
        : cascade_index == 1 ? ubo.u_shadow_splits.y
        : cascade_index == 2 ? ubo.u_shadow_splits.z
        : ubo.u_shadow_splits.w;
    float split_near = cascade_index == 0 ? 0.0
        : cascade_index == 1 ? ubo.u_shadow_splits.x
        : cascade_index == 2 ? ubo.u_shadow_splits.y
        : ubo.u_shadow_splits.z;
    float split_band = max((split_far - split_near) * 0.08, 2.0);
    cascade_fade = 1.0 - smoothstep(split_far - split_band, split_far, view_distance);

    int columns = 2;
    int rows = cascade_count <= 2 ? 1 : 2;
    float inv_columns = 1.0 / float(columns);
    float inv_rows = 1.0 / float(rows);
    tile_scale = vec2(inv_columns, inv_rows);
    tile_offset = vec2(float(cascade_index % columns) * inv_columns, float(cascade_index / columns) * inv_rows);
    return ubo.u_cascade_light_mvp[cascade_index] * vec4(wpos, 1.0);
}

float sample_shadow(vec4 fallback_light_clip, vec3 nrm, vec3 light_dir_to_scene) {
    vec2 tile_offset;
    vec2 tile_scale;
    float cascade_fade;
    vec4 light_clip = shadow_clip_for_receiver(v_wpos, tile_offset, tile_scale, cascade_fade);
    if (ubo.u_shadow_params.x < 0.5 || light_clip.w <= 0.0) {
        return 1.0;
    }

    vec3 ndc = light_clip.xyz / light_clip.w;
    vec2 local_uv = ndc.xy * 0.5 + 0.5;
    if (local_uv.x < 0.001 || local_uv.x > 0.999 || local_uv.y < 0.001 || local_uv.y > 0.999 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }

    ivec2 atlas_size = textureSize(sampler2D(u_shadow_tex, u_material_sampler), 0);
    vec2 atlas_texel = 1.0 / vec2(max(atlas_size.x, 1), max(atlas_size.y, 1));
    vec2 guard = max(atlas_texel / max(tile_scale, vec2(0.0001)), vec2(0.0015));
    vec2 atlas_uv = tile_offset + clamp(local_uv, guard, vec2(1.0) - guard) * tile_scale;

    float current = ndc.z;
    float ndotl = max(dot(normalize(nrm), normalize(-light_dir_to_scene)), 0.0);
    float slope = 1.0 - ndotl;
    float receiver_bias = ubo.u_shadow_params.y * (1.0 + slope * 2.85);
    float normal_bias = clamp(ubo.u_shadow_extra.x, 0.0, 0.006) * slope;
    float bias = max(receiver_bias + normal_bias, 0.00005);
    float strength = clamp(ubo.u_shadow_params.z, 0.0, 0.78);

    float lit = shadow_compare_quality(atlas_uv, current, bias);
    float border = min(min(local_uv.x, local_uv.y), min(1.0 - local_uv.x, 1.0 - local_uv.y));
    float edge_fade = smoothstep(0.010, 0.070, border)
        * smoothstep(0.010, 0.080, ndc.z)
        * (1.0 - smoothstep(0.920, 0.992, ndc.z))
        * cascade_fade;
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


vec3 daylight_guard_ambient(vec3 base, vec3 normal_ws, vec3 view_ws, float roughness, float metallic, float occlusion, out bool guard_active) {
    float ambient_energy = ne_luma(abs(ubo.u_ambient.rgb)) * max(ubo.u_ambient.a, 0.0);
    float sun_energy = ne_luma(abs(ubo.u_dir_color.rgb)) * max(ubo.u_dir_dir_intensity.w, 0.0);
    guard_active = ambient_energy < 0.0025 && sun_energy < 0.0025;
    if (!guard_active) {
        return vec3(0.0);
    }

    // Failsafe for legacy material UBOs: if a scene has TOD-driven sky/terrain
    // but an opaque material receives zero ambient and zero sun, keep it visible
    // with the same daytime floor used by GameReady bootstrapping. This mirrors
    // professional shader stacks that separate black-level protection from final
    // tone mapping instead of allowing valid albedo to collapse to black.
    float up = saturate(normal_ws.y * 0.5 + 0.5);
    vec3 sky = vec3(0.38, 0.42, 0.50) * 0.28 * mix(0.78, 1.16, up);
    vec3 ground = vec3(0.20, 0.18, 0.15) * 0.28;
    vec3 ambient = mix(ground, sky, up) * base * max(occlusion, 0.35);

    vec3 L = normalize(vec3(0.53590363, 0.7989835, 0.27282366));
    vec3 direct = pbr_direct(base, normal_ws, view_ws, L, vec3(1.0, 0.94, 0.82), 2.35, roughness, metallic);
    vec3 F0 = mix(vec3(0.04), base, metallic);
    vec3 rim = fresnel_schlick_roughness(max(dot(normal_ws, view_ws), 0.0), F0, roughness) * sky * 0.10;
    return max(ambient + direct + rim, base * 0.06);
}

vec3 sky_dome_radiance(vec3 sampled, vec3 tint, vec3 view_dir, vec3 emissive_color) {
    vec3 to_sun = normalize(-ubo.u_dir_dir_intensity.xyz);
    vec3 to_moon = -to_sun;
    float sun_elevation = to_sun.y;
    float day = smoothstep(-0.10, 0.22, sun_elevation);
    float night = 1.0 - smoothstep(-0.12, 0.18, sun_elevation);
    float twilight = (1.0 - smoothstep(0.10, 0.56, abs(sun_elevation))) * smoothstep(-0.24, 0.10, sun_elevation);
    float horizon = pow(saturate(1.0 - abs(view_dir.y)), 1.65);

    vec3 night_zenith = vec3(0.006, 0.010, 0.030);
    vec3 day_zenith = vec3(0.18, 0.38, 0.82);
    vec3 dusk_zenith = vec3(0.14, 0.17, 0.34);
    vec3 zenith = mix(mix(night_zenith, day_zenith, day), dusk_zenith, saturate(twilight * (1.0 - day * 0.45)));

    vec3 sunset = vec3(1.0, 0.47, 0.20);
    vec3 day_horizon = mix(vec3(0.58, 0.75, 0.98), sunset, saturate(twilight));
    vec3 night_horizon = vec3(0.018, 0.022, 0.050);
    vec3 atmosphere = mix(zenith, mix(night_horizon, day_horizon, day + twilight * 0.55), horizon);

    // CPU sky-cycle writes v_base as the authoritative TOD tint. The shader
    // multiplies procedural atmosphere by that tint, so sky luminance stays
    // coupled to terrain lighting instead of remaining bright during night.
    vec3 runtime_tint = max(tint, vec3(0.002));
    float runtime_luma = max(ne_luma(runtime_tint), 0.002);
    vec3 runtime_chroma = runtime_tint / runtime_luma;
    float runtime_darkening = clamp(runtime_luma * 2.1, 0.035, 1.35);
    atmosphere *= mix(vec3(runtime_darkening), runtime_chroma * runtime_darkening, 0.42);

    vec3 cloud_sample = material_texture_safe(sampled, tint, vec3(0.42, 0.46, 0.54));
    vec2 wind_phase = vec2(to_sun.x + to_moon.z * 0.25, to_sun.z - to_moon.x * 0.25);
    vec2 cloud_uv_0 = v_uv * vec2(1.00, 0.58) + wind_phase * vec2(0.035, 0.012);
    vec2 cloud_uv_1 = v_uv * vec2(1.87, 0.91) - wind_phase.yx * vec2(0.020, 0.026);
    vec3 cloud_motion_0 = textureGrad(sampler2D(u_base_tex, u_material_sampler), cloud_uv_0, dFdx(cloud_uv_0), dFdy(cloud_uv_0)).rgb;
    vec3 cloud_motion_1 = textureGrad(sampler2D(u_base_tex, u_material_sampler), cloud_uv_1, dFdx(cloud_uv_1), dFdy(cloud_uv_1)).rgb;
    float density_src = max(max(cloud_sample.r, cloud_sample.g), cloud_sample.b) * 0.58
        + max(max(cloud_motion_0.r, cloud_motion_0.g), cloud_motion_0.b) * 0.27
        + max(max(cloud_motion_1.r, cloud_motion_1.g), cloud_motion_1.b) * 0.15;
    float density = smoothstep(0.34, 0.78, density_src * 1.22);
    vec3 cloud_day = vec3(1.0, 0.965, 0.88);
    vec3 cloud_dusk = vec3(1.0, 0.58, 0.32);
    vec3 cloud_night = vec3(0.040, 0.050, 0.085);
    vec3 cloud_color = mix(mix(cloud_night, cloud_day, day), cloud_dusk, twilight * 0.42)
        * density
        * (0.12 + 0.72 * horizon)
        * clamp(runtime_darkening * 1.18, 0.04, 1.25);

    float sun_dot = saturate(dot(view_dir, to_sun));
    float disk = pow(sun_dot, 4096.0) * 3.6;
    float halo = pow(sun_dot, 64.0) * 0.20 + pow(sun_dot, 12.0) * 0.030;
    vec3 sun = ubo.u_dir_color.rgb * max(ubo.u_dir_dir_intensity.w, 0.0) * (day + twilight * 0.22) * (disk + halo);

    float moon_visibility = night * smoothstep(-0.08, 0.18, to_moon.y);
    float moon_dot = saturate(dot(view_dir, to_moon));
    vec3 moon = vec3(0.42, 0.48, 0.68) * moon_visibility
        * (pow(moon_dot, 768.0) * 0.72 + pow(moon_dot, 48.0) * 0.080 + pow(moon_dot, 9.0) * 0.018);

    vec3 stars = vec3(0.55, 0.62, 0.82) * pow(saturate(view_dir.y), 2.2) * pow(night, 2.0) * 0.018;

    return max(atmosphere + cloud_color + sun + moon + stars + emissive_color * 0.08 * runtime_darkening, vec3(0.0));
}

void main() {
    vec2 stable_material_uv_dx = dFdx(v_uv);
    vec2 stable_material_uv_dy = dFdy(v_uv);
    vec3 N = safe_normalize(v_wnrm, vec3(0.0, 1.0, 0.0));
    vec4 material_params = ubo.u_material_params;
    vec4 emissive_color = ubo.u_emissive;

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

    bool daylight_guard_active = false;
    vec3 color = daylight_guard_ambient(base, N, V, roughness, metallic, occlusion, daylight_guard_active);
    if (!daylight_guard_active) {
        color = hemi_ambient(base, N, V, roughness, metallic, occlusion);
    }

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

    // Debug-safe black floor: never let a valid non-sky material with resolved
    // albedo become physically invisible because every light input was missing.
    if (daylight_guard_active) {
        color = max(color, base * 0.08);
    }

    o_color = vec4(max(color, vec3(0.0)), v_base.a * texel.a);
}
