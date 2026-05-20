#version 450
layout(std430, set = 2, binding = 2) readonly buffer TreeImposterMetadataBuffer { vec4 imposter_pos_radius[]; };
layout(location = 0) in vec2 in_corner;
layout(location = 1) in vec2 in_uv;
layout(push_constant) uniform TreeImposterPush { mat4 view_proj; vec4 camera_right_radius; vec4 camera_up_lod; } pc;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out float v_lod_fade;
void main() {
    vec4 imp = imposter_pos_radius[gl_InstanceIndex];
    vec3 world = imp.xyz + pc.camera_right_radius.xyz * in_corner.x * imp.w + pc.camera_up_lod.xyz * in_corner.y * imp.w;
    v_uv = in_uv;
    v_lod_fade = pc.camera_up_lod.w;
    gl_Position = pc.view_proj * vec4(world, 1.0);
}
