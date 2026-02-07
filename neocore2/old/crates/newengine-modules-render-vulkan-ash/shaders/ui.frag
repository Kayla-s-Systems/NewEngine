#version 450

layout(set = 0, binding = 0) uniform sampler2D u_tex;

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

layout(location = 0) out vec4 o_color;

void main() {
    vec4 t = texture(u_tex, v_uv);
    o_color = t * v_color;
}