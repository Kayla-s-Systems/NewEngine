#version 450

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;

layout(location = 0) out vec2 v_uv;

layout(push_constant) uniform ParaboloidShadowPush {
    mat4 model;
    vec4 light_position_radius;
    vec4 near_far_face_bias;
} pc;

void main() {
    vec4 world = pc.model * vec4(a_position, 1.0);
    vec3 local = world.xyz - pc.light_position_radius.xyz;
    float len = max(length(local), 0.0001);
    vec3 dir = local / len;
    float face = pc.near_far_face_bias.z >= 0.0 ? 1.0 : -1.0;
    float m = 1.0 / max(1.0 + dir.z * face, 0.0001);
    gl_Position = vec4(dir.xy * m, (len - pc.near_far_face_bias.x) / max(pc.near_far_face_bias.y - pc.near_far_face_bias.x, 0.0001), 1.0);
    v_uv = a_uv;
}
