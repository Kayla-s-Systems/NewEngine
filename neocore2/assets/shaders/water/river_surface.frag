#version 450
#include "water_common.glsl"
layout(set = 0, binding = 0) uniform sampler2D u_reflection;
layout(set = 0, binding = 1) uniform sampler2D u_refraction;
layout(set = 0, binding = 2) uniform sampler2D u_depth;
layout(set = 0, binding = 3) uniform sampler2D u_flow_noise;
layout(push_constant) uniform RiverPush { mat4 world_view_proj; vec4 time_reflection; vec4 flow; vec4 material; } pc;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_world_normal;
layout(location = 0) out vec4 out_color;
void main() {
    float time = pc.time_reflection.x;
    vec2 flow_dir = normalize(pc.flow.xy + vec2(1e-4));
    vec2 uv = v_uv + flow_dir * time * pc.flow.z;
    vec4 noise = texture(u_flow_noise, uv * 8.0);
    vec2 distortion = (noise.xy * 2.0 - 1.0) * pc.material.z;
    vec3 reflected = texture(u_reflection, v_uv + distortion).rgb;
    vec3 refracted = texture(u_refraction, v_uv - distortion * 0.25).rgb;
    float depth_delta = texture(u_depth, v_uv).r * 10.0;
    float foam = ne_shoreline_foam(depth_delta, noise.z, pc.material.w);
    float fresnel = ne_fresnel_schlick(abs(normalize(v_world_normal + vec3(distortion, 0.6)).z), pc.material.y, 4.0);
    vec3 color = mix(refracted * vec3(0.55, 0.82, 0.72), reflected, fresnel);
    color = mix(color, vec3(0.92, 0.96, 0.9), foam);
    out_color = vec4(color, 0.72);
}
