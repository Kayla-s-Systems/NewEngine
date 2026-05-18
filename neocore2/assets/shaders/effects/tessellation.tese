#version 450
#include "common.glsl"

#ifndef TESS_DISPLACEMENT_SCALE
#define TESS_DISPLACEMENT_SCALE 0.000000
#endif

layout(triangles, equal_spacing, ccw) in;

layout(location = 0) in vec3 tc_position[];
layout(location = 1) in vec3 tc_normal[];
layout(location = 2) in vec2 tc_uv[];

layout(push_constant) uniform TessEvalPushConstants {
    mat4 view_proj;
} pc;

layout(location = 0) out vec3 te_position;
layout(location = 1) out vec3 te_normal;
layout(location = 2) out vec2 te_uv;

void main() {
    vec3 p = tc_position[0] * gl_TessCoord.x + tc_position[1] * gl_TessCoord.y + tc_position[2] * gl_TessCoord.z;
    vec3 n = normalize(tc_normal[0] * gl_TessCoord.x + tc_normal[1] * gl_TessCoord.y + tc_normal[2] * gl_TessCoord.z);
    vec2 uv = tc_uv[0] * gl_TessCoord.x + tc_uv[1] * gl_TessCoord.y + tc_uv[2] * gl_TessCoord.z;
    p += n * float(TESS_DISPLACEMENT_SCALE);
    te_position = p;
    te_normal = n;
    te_uv = uv;
    gl_Position = pc.view_proj * vec4(p, 1.0);
}
