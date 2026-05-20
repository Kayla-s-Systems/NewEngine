#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(push_constant) uniform WaterSurfacePush {
    mat4 world_view_proj;
    vec4 time_reflection; // x=time, y=reflectionReady, z=refractionReady, w=surfaceClass
    vec4 normal_params;   // xy speed A, zw speed B
    vec4 material;        // x roughness, y fresnelBias, z distortion, w foamCutoff
} pc;

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec3 v_world_normal;
layout(location = 2) out vec4 v_params;

void main() {
    gl_Position = pc.world_view_proj * vec4(in_pos, 1.0);
    v_uv = in_uv;
    v_world_normal = normalize(in_normal);
    v_params = vec4(pc.time_reflection.x, pc.material.xyz);
}
