#version 450
layout(set = 0, binding = 0) uniform sampler2D u_reflection_msaa_or_color;
layout(set = 0, binding = 1) uniform sampler2D u_depth;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;
void main() {
    vec3 color = texture(u_reflection_msaa_or_color, v_uv).rgb;
    float depth = texture(u_depth, v_uv).r;
    float horizon_fade = smoothstep(0.0, 0.08, v_uv.y) * smoothstep(1.0, 0.75, v_uv.y);
    out_color = vec4(color * horizon_fade, depth < 1.0 ? 1.0 : 0.0);
}
