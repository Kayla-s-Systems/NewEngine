#version 450

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

layout(push_constant) uniform CsmDebugPush {
    vec4 cascade_color;
    vec4 atlas_rect;
} pc;

void main() {
    vec2 edge = min(v_uv, 1.0 - v_uv);
    float line = step(edge.x, 0.01) + step(edge.y, 0.01);
    float alpha = clamp(line, 0.0, 1.0) * pc.cascade_color.a;
    o_color = vec4(pc.cascade_color.rgb, alpha);
}
