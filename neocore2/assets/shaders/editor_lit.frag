#version 450
layout (location = 0) in vec3 v_nrm;
layout (location = 0) out vec4 o_col;

layout (set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
    vec4 u_base_color;
} u;

void main() {
    vec3 n = normalize(v_nrm);
    vec3 l = normalize(vec3(0.35, 0.75, 0.55));
    float ndl = clamp(dot(n, l) * 0.5 + 0.5, 0.0, 1.0);
    o_col = vec4(u.u_base_color.rgb * ndl, u.u_base_color.a);
}
