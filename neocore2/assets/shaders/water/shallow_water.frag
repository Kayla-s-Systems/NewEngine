#version 450
#include "water_common.glsl"
layout(set = 0, binding = 0) uniform sampler2D u_reflection;
layout(set = 0, binding = 1) uniform sampler2D u_refraction;
layout(set = 0, binding = 2) uniform sampler2D u_depth;
layout(set = 0, binding = 3) uniform sampler2D u_noise;
layout(push_constant) uniform ShallowPush { mat4 world_view_proj; vec4 time_reflection; vec4 normal_params; vec4 material; } pc;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_world_normal;
layout(location = 0) out vec4 out_color;
void main() {
    float depth_delta = texture(u_depth, v_uv).r * 6.0;
    vec3 n = texture(u_noise, v_uv * 12.0 + pc.normal_params.xy * pc.time_reflection.x).xyz * 2.0 - 1.0;
    vec2 distortion = n.xy * pc.material.z;
    vec3 refl = texture(u_reflection, v_uv + distortion).rgb;
    vec3 refr = texture(u_refraction, v_uv - distortion).rgb;
    float fresnel = ne_fresnel_schlick(abs(normalize(v_world_normal + n * 0.25).z), pc.material.y, 4.0);
    float foam = ne_shoreline_foam(depth_delta, n.z * 0.5 + 0.5, pc.material.w);
    vec3 color = mix(refr * vec3(0.86, 0.96, 0.90), refl, fresnel * 0.55);
    out_color = vec4(mix(color, vec3(0.96), foam), 0.55);
}
