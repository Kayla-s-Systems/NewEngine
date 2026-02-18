#version 450

layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNrm;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 uMvp;
    vec4 uColor;
    vec4 uLightDir;
} u;

layout(location = 0) out vec3 vNrm;
layout(location = 1) out vec4 vColor;

void main() {
    gl_Position = u.uMvp * vec4(aPos, 1.0);
    vNrm = aNrm;
    vColor = u.uColor;
}
