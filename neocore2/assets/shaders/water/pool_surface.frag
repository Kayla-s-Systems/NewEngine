#version 450
#include "water_common.glsl"
layout(set = 0, binding = 0) uniform sampler2D u_reflection;
layout(set = 0, binding = 1) uniform sampler2D u_refraction;
layout(set = 0, binding = 2) uniform sampler2D u_depth;
layout(set = 0, binding = 3) uniform sampler2D u_ripple;
layout(push_constant) uniform PoolPush { mat4 world_view_proj; vec4 time_reflection; vec4 ripple; vec4 material; } pc;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_world_normal;
layout(location = 0) out vec4 out_color;
void main() {
    float t = pc.time_reflection.x;
    vec3 ripple = texture(u_ripple, v_uv * 5.0 + pc.ripple.xy * t).xyz * 2.0 - 1.0;
    vec2 distortion = ripple.xy * pc.material.z;
    float fresnel = ne_fresnel_schlick(abs(normalize(v_world_normal + ripple * 0.2).z), pc.material.y, 6.5);
    vec3 refl = texture(u_reflection, v_uv + distortion).rgb;
    vec3 refr = texture(u_refraction, v_uv - distortion * 0.15).rgb;
    out_color = vec4(mix(refr * vec3(0.72, 0.92, 1.0), refl, fresnel), 0.62);
}
