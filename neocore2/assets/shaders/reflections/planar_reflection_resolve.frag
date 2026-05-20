#version 450
layout(set = 0, binding = 0) uniform sampler2D u_probe;
layout(set = 0, binding = 1) uniform sampler2D u_depth;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;
void main() {
    vec3 probe = texture(u_probe, v_uv).rgb;
    float depth = texture(u_depth, v_uv).r;
    out_color = vec4(probe, smoothstep(1.0, 0.2, depth));
}
