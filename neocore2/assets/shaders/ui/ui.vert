#version 450

layout(location = 0) in vec2 a_pos_px;
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;

layout(push_constant) uniform Pc {
    vec2 screen_size;
    vec2 _pad;
} pc;

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

void main() {
    vec2 ndc = (a_pos_px / pc.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}