#ifndef NEWENGINE_REFLECTION_COMMON_GLSL
#define NEWENGINE_REFLECTION_COMMON_GLSL
vec2 ne_reflection_project(vec4 clip_pos) {
    vec2 ndc = clip_pos.xy / max(clip_pos.w, 1e-5);
    return ndc * 0.5 + 0.5;
}
float ne_clip_plane_distance(vec3 world_pos, vec4 plane) {
    return dot(world_pos, plane.xyz) + plane.w;
}
#endif
