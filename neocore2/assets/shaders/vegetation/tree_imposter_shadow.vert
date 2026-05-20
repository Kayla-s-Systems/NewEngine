#version 450
layout(std430, set = 2, binding = 2) readonly buffer TreeImposterMetadataBuffer { vec4 imposter_pos_radius[]; };
layout(location = 0) in vec2 in_corner;
layout(location = 1) in vec2 in_uv;
layout(push_constant) uniform TreeImposterShadowPush { mat4 light_view_proj; vec4 right_radius; vec4 up_fade; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out float v_fade;
void main() {
    vec4 imp = imposter_pos_radius[gl_InstanceIndex];
    vec3 world = imp.xyz + pc.right_radius.xyz * in_corner.x * imp.w + pc.up_fade.xyz * in_corner.y * imp.w;
    v_uv = in_uv;
    v_fade = pc.up_fade.w;
    gl_Position = pc.light_view_proj * vec4(world, 1.0);
}
