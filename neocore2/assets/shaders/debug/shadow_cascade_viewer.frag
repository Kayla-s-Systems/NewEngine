#version 450
layout(location=0) in vec2 v_uv;
layout(location=0) out vec4 o_color;
layout(set=0,binding=0) uniform sampler2D u_shadow_atlas;
layout(push_constant) uniform DebugShadowPush { vec4 p[2]; } pc;
void main() {
    float d = texture(u_shadow_atlas, v_uv).r;
    float valid = step(0.0005, d) * (1.0 - step(0.9995, d));
    vec3 depth_vis = mix(vec3(0.08,0.12,0.18), vec3(d), valid);
    vec2 grid = abs(fract(v_uv * vec2(2.0,2.0)) - 0.5);
    float line = 1.0 - smoothstep(0.0, 0.01, min(grid.x, grid.y));
    o_color = vec4(mix(depth_vis, vec3(1.0,0.82,0.08), line), 1.0);
}
