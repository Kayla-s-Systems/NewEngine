#version 450
#include "common.glsl"

layout(set = 0, binding = 0) uniform sampler2D u_scene;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

void main() {
    vec4 c = texture(u_scene, ne_clamp_uv(v_uv));
    o_color = vec4(ne_safe_color(c.rgb), c.a);
}
