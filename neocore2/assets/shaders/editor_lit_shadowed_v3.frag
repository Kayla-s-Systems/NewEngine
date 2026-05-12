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
    // x: enabled, y: base bias, z: shadow/contact strength, w: PCF softness radius
    vec4 u_shadow_params;
} ubo;
layout(set = 0, binding = 1) uniform texture2D u_base_tex;
layout(set = 0, binding = 2) uniform texture2D u_normal_tex;
layout(set = 0, binding = 3) uniform texture2D u_roughness_tex;
layout(set = 0, binding = 4) uniform texture2D u_shadow_tex;
layout(set = 0, binding = 5) uniform sampler u_material_sampler;

layout(location = 0) out vec4 o_color;

const float PI = 3.14159265359;

mat3 cotangent_frame(vec3 n, vec3 p, vec2 uv) {
    vec3 dp1 = dFdx(p);
    vec3 dp2 = dFdy(p);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, n);
    vec3 dp1perp = cross(n, dp1);
    vec3 t = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 b = dp2perp * duv1.y + dp1perp * duv2.y;
    float invmax = inversesqrt(max(dot(t, t), dot(b, b)) + 1.0e-8);
    return mat3(t * invmax, b * invmax, n);
}

float shadow_tap(vec2 uv, float current, float bias) {
    float closest = texture(sampler2D(u_shadow_tex, u_material_sampler), clamp(uv, vec2(0.001), vec2(0.999))).r;
    if (closest >= 0.9995) {
        return 1.0;
    }
    return (current - bias <= closest) ? 1.0 : 0.0;
}

float shadow_compare_stable(vec2 uv, float current, float bias) {
    ivec2 sz = textureSize(sampler2D(u_shadow_tex, u_material_sampler), 0);
    vec2 texel = clamp(ubo.u_shadow_params.w, 0.0, 1.25) / vec2(max(sz.x, 1), max(sz.y, 1));

    // Stable 3-tap PCF. The previous dual-origin min() comparison made shadows
    // visible, but it also shadowed unrelated mirrored texels and produced the
    // noisy black speckle pattern seen on large terrain surfaces. We now use the
    // Vulkan render-target origin convention explicitly and keep the filter small
    // until the renderer grows a true depth-comparison sampler/CSM chain.
    float lit = 0.0;
    lit += shadow_tap(uv, current, bias) * 0.50;
    lit += shadow_tap(uv + vec2(texel.x, 0.0), current, bias) * 0.25;
    lit += shadow_tap(uv + vec2(0.0, texel.y), current, bias) * 0.25;
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
    float bias = ubo.u_shadow_params.y * (1.0 + slope * 2.25);
    float strength = clamp(ubo.u_shadow_params.z, 0.0, 0.70);

    float lit = shadow_compare_stable(uv, current, bias);
    return mix(1.0 - strength, 1.0, lit);
}

float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return a2 / max(PI * denom * denom, 1.0e-5);
}

float geometry_schlick_ggx(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdotV / max(NdotV * (1.0 - k) + k, 1.0e-5);
}

float geometry_smith(vec3 N, vec3 V, vec3 L, float roughness) {
    float ggx1 = geometry_schlick_ggx(max(dot(N, V), 0.0), roughness);
    float ggx2 = geometry_schlick_ggx(max(dot(N, L), 0.0), roughness);
    return ggx1 * ggx2;
}

vec3 fresnel_schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 pbr_direct(vec3 base, vec3 N, vec3 V, vec3 L, vec3 light_color, float intensity, float roughness, float metallic) {
    vec3 H = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotL <= 0.0 || NdotV <= 0.0) {
        return vec3(0.0);
    }

    vec3 F0 = mix(vec3(0.04), base, metallic);
    float D = distribution_ggx(N, H, roughness);
    float G = geometry_smith(N, V, L, roughness);
    vec3 F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    vec3 specular = (D * G * F) / max(4.0 * NdotV * NdotL, 1.0e-4);
    vec3 kD = (vec3(1.0) - F) * (1.0 - metallic);
    vec3 diffuse = kD * base / PI;
    return (diffuse + specular) * light_color * intensity * NdotL;
}

void main() {
    vec2 stable_material_uv_dx = dFdx(v_uv);
    vec2 stable_material_uv_dy = dFdy(v_uv);
    vec3 N = normalize(v_wnrm);
    float normal_scale = clamp(ubo.u_material_params.x, 0.0, 1.0);
    if (normal_scale > 0.001) {
        vec3 map_n = textureGrad(sampler2D(u_normal_tex, u_material_sampler), v_uv, stable_material_uv_dx, stable_material_uv_dy).xyz * 2.0 - 1.0;
        mat3 tbn = cotangent_frame(N, v_wpos, v_uv);
        N = normalize(tbn * vec3(map_n.xy * normal_scale, map_n.z));
    }

    vec4 texel = textureGrad(sampler2D(u_base_tex, u_material_sampler), v_uv, stable_material_uv_dx, stable_material_uv_dy);
    vec3 base = clamp((v_base * texel).rgb, vec3(0.0), vec3(1.0));
    float roughness_sample = textureGrad(sampler2D(u_roughness_tex, u_material_sampler), v_uv, stable_material_uv_dx, stable_material_uv_dy).r;
    float roughness = clamp(ubo.u_material_params.y * roughness_sample, 0.02, 1.0);
    float metallic = clamp(ubo.u_material_params.z, 0.0, 1.0);
    float occlusion = clamp(ubo.u_material_params.w, 0.0, 1.0);

    // Until camera position is part of the lit UBO, use a stable approximate view vector.
    vec3 V = normalize(-v_wpos);
    if (dot(V, V) <= 1.0e-6) {
        V = vec3(0.0, 0.0, 1.0);
    }

    vec3 color = ubo.u_ambient.rgb * ubo.u_ambient.a * base * occlusion;

    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float shadow = sample_shadow(v_light_clip, N, ubo.u_dir_dir_intensity.xyz);
    color += shadow * pbr_direct(
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
        float atten = clamp(1.0 - (dist / range), 0.0, 1.0);
        atten *= atten;
        color += atten * pbr_direct(
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

    color += ubo.u_emissive.rgb;
    o_color = vec4(color, v_base.a * texel.a);
}
