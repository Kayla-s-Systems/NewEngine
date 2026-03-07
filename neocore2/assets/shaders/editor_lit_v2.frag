#version 450

layout (location = 0) in vec3 v_wpos;
layout (location = 1) in vec3 v_wnrm;
layout (location = 2) in vec4 v_base;

layout (set = 0, binding = 0, std140) uniform Ubo {
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
} ubo;

layout (location = 0) out vec4 o_color;

float saturate(float x) { return clamp(x, 0.0, 1.0); }

void main() {
    vec3 N = normalize(v_wnrm);
    vec3 base = v_base.rgb;
    vec3 emissive = ubo.u_emissive.rgb;

    vec3 lit = ubo.u_ambient.rgb * ubo.u_ambient.a;

    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float NdL = max(dot(N, Ld), 0.0);
    lit += ubo.u_dir_color.rgb * (ubo.u_dir_dir_intensity.a * NdL);

    int n = int(ubo.u_point_count_pad.x + 0.5);
    n = clamp(n, 0, 4);
    for (int i = 0; i < n; i++) {
        vec3 P = ubo.u_point_pos_range[i].xyz;
        float range = max(ubo.u_point_pos_range[i].w, 0.001);
        vec3 toL = P - v_wpos;
        float d2 = dot(toL, toL);
        float d = sqrt(max(d2, 1e-6));
        vec3 L = toL / d;
        float att = 1.0 / max(d2, 1e-4);
        float fade = 1.0 - saturate(d / range);
        float NdLp = max(dot(N, L), 0.0);
        vec3 col = ubo.u_point_color_intensity[i].rgb;
        float inten = ubo.u_point_color_intensity[i].a;
        lit += col * (inten * NdLp * att * fade * fade);
    }

    vec3 out_rgb = base * lit + emissive;
    o_color = vec4(out_rgb, v_base.a);
}