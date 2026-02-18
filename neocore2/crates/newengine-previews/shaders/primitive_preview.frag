#version 450

layout(location = 0) in vec3 vNrm;
layout(location = 1) in vec4 vColor;

layout(location = 0) out vec4 oColor;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 uMvp;
    vec4 uColor;
    vec4 uLightDir;
} u;

void main() {
    vec3 n = normalize(vNrm);
    vec3 l = normalize(u.uLightDir.xyz);

    float ndl = max(dot(n, l), 0.0);
    float amb = 0.22;
    float diff = 0.78 * ndl;

    vec3 base = vColor.rgb;
    vec3 col = base * (amb + diff);

    // Slight rim
    float rim = pow(1.0 - max(n.z, 0.0), 2.0);
    col += 0.08 * rim;

    oColor = vec4(col, vColor.a);
}
