#version 450
layout(set = 0, binding = 0) uniform sampler2D u_reflection;
layout(set = 0, binding = 1) uniform sampler2D u_crack;
layout(push_constant) uniform MirrorPush { mat4 world_view_proj; vec4 mirror_plane; vec4 mirror_params; } pc;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in float v_clip;
layout(location = 0) out vec4 out_color;
void main() {
    if (v_clip < -0.0005) { discard; }
    vec2 crack = texture(u_crack, v_uv * pc.mirror_params.zw).xy * 2.0 - 1.0;
    vec3 refl = texture(u_reflection, v_uv + crack * pc.mirror_params.y).rgb;
    out_color = vec4(refl * pc.mirror_params.x, 1.0);
}
