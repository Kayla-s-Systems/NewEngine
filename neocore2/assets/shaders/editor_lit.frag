#version 450

layout (location = 0) in vec3 v_pos_ws;
layout (location = 1) in vec3 v_nrm_ws;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_ambient;            // rgb + intensity
    vec4 u_dir_dir_intensity;  // xyz direction (incoming rays) + intensity
    vec4 u_dir_color;          // rgb
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;    // x = count
} ubo;

layout (location = 0) out vec4 out_color;

float saturate(float x) { return clamp(x, 0.0, 1.0); }

void main() {
    vec3 n = normalize(v_nrm_ws);
    vec3 base = ubo.u_base_color.rgb;

    vec3 ambient = ubo.u_ambient.rgb * ubo.u_ambient.w;

    vec3 lit = vec3(0.0);

    // Directional light.
    vec3 dir = normalize(ubo.u_dir_dir_intensity.xyz);
    float dir_int = ubo.u_dir_dir_intensity.w;
    vec3 dir_col = ubo.u_dir_color.rgb * dir_int;
    // Incoming rays direction -> surface-to-light is opposite.
    vec3 Ld = normalize(-dir);
    float ndotl = max(dot(n, Ld), 0.0);
    lit += dir_col * ndotl;

    // Point lights.
    int count = int(ubo.u_point_count_pad.x + 0.5);
    count = clamp(count, 0, 4);
    for (int i = 0; i < count; ++i) {
        vec3 lp = ubo.u_point_pos_range[i].xyz;
        float range = max(ubo.u_point_pos_range[i].w, 1e-3);

        vec3 L = lp - v_pos_ws;
        float dist2 = dot(L, L);
        float dist = sqrt(max(dist2, 1e-6));
        vec3 Ln = L / dist;

        // Smooth quadratic falloff inside range.
        float t = saturate(1.0 - dist / range);
        float att = t * t;

        vec3 plc = ubo.u_point_color_intensity[i].rgb;
        float pli = max(ubo.u_point_color_intensity[i].w, 0.0);

        float nlp = max(dot(n, Ln), 0.0);
        lit += plc * (pli * att * nlp);
    }

    vec3 rgb = base * (ambient + lit);
    out_color = vec4(rgb, ubo.u_base_color.a);
}
