#version 450
layout(set = 0, binding = 0, std140) uniform DebugLineUbo {
    vec4 u_pad;
} ubo;

layout(location = 0) in vec4 a_clip_pos;
layout(location = 1) in vec4 a_color;
layout(location = 0) out vec4 v_color;
void main() {
    gl_Position = a_clip_pos + vec4(ubo.u_pad.xyz * 0.0, 0.0);
    v_color = a_color;
}
