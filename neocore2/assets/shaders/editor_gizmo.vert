#version 450

// Editor gizmo vertex shader.
// Note: current editor gizmo is drawn as a 2D egui overlay.
// This shader is staged for the future GPU gizmo overlay pipeline.

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_color;

layout(set = 0, binding = 0) uniform Ubo {
    mat4 u_mvp;
} u;

layout(location = 0) out vec3 v_color;

void main() {
    v_color = a_color;
    gl_Position = u.u_mvp * vec4(a_pos, 1.0);
}
