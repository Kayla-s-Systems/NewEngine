#version 450

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;
layout(location = 3) in vec2 v_uv;

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
    vec4 u_material_params;
} ubo;
layout(set = 0, binding = 1) uniform texture2D u_base_tex;
layout(set = 0, binding = 2) uniform texture2D u_normal_tex;
layout(set = 0, binding = 3) uniform texture2D u_roughness_tex;
layout(set = 0, binding = 4) uniform sampler u_material_sampler;

layout(location = 0) out vec4 o_color;

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

void main() {
    vec3 N = normalize(v_wnrm);
    vec3 map_n = texture(sampler2D(u_normal_tex, u_material_sampler), v_uv).xyz * 2.0 - 1.0;
    mat3 tbn = cotangent_frame(N, v_wpos, v_uv);
    float normal_scale = max(ubo.u_material_params.x, 0.0);
    N = normalize(tbn * vec3(map_n.xy * normal_scale, map_n.z));

    vec4 texel = texture(sampler2D(u_base_tex, u_material_sampler), v_uv);
    float roughness_sample = texture(sampler2D(u_roughness_tex, u_material_sampler), v_uv).r;
    float roughness = clamp(ubo.u_material_params.y * roughness_sample, 0.02, 1.0);
    vec3 base = (v_base * texel).rgb;
    vec3 emissive = ubo.u_emissive.rgb;

    vec3 lit = ubo.u_ambient.rgb * ubo.u_ambient.a;
    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float NdL = max(dot(N, Ld), 0.0);
    float rough_diffuse = mix(1.12, 0.82, roughness);
    lit += NdL * rough_diffuse * ubo.u_dir_color.rgb * ubo.u_dir_dir_intensity.w;

    int point_count = int(ubo.u_point_count_pad.x + 0.5);
    for (int i = 0; i < point_count && i < 4; ++i) {
        vec3 toL = ubo.u_point_pos_range[i].xyz - v_wpos;
        float dist = length(toL);
        float range = max(ubo.u_point_pos_range[i].w, 0.0001);
        vec3 L = toL / max(dist, 0.0001);
        float atten = clamp(1.0 - (dist / range), 0.0, 1.0);
        float ndl = max(dot(N, L), 0.0);
        lit += ndl * atten * rough_diffuse * ubo.u_point_color_intensity[i].rgb * ubo.u_point_color_intensity[i].w;
    }

    vec3 color = base * lit + emissive;
    o_color = vec4(color, v_base.a * texel.a);
}
