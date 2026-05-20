#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(push_constant) uniform MirrorPush { mat4 world_view_proj; vec4 mirror_plane; vec4 mirror_params; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out float v_clip;
void main() {
    gl_Position = pc.world_view_proj * vec4(in_pos, 1.0);
    v_uv = in_uv;
    v_clip = dot(in_pos, pc.mirror_plane.xyz) + pc.mirror_plane.w;
}
