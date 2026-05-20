#version 450
layout(location=0) in vec2 v_uv;
layout(location=0) out vec4 o_color;
layout(set=0,binding=0) uniform sampler2D u_target;
layout(push_constant) uniform DebugTargetPush { vec4 p[2]; } pc;
vec3 tonemap_debug(vec3 c) { return c / (c + vec3(1.0)); }
void main() {
    vec3 c = texture(u_target, v_uv).rgb;
    int mode = int(pc.p[0].x + 0.5);
    if (mode == 1) c = vec3(dot(c, vec3(0.2126,0.7152,0.0722)));
    if (mode == 2) c = abs(c);
    o_color = vec4(tonemap_debug(max(c, vec3(0.0))), 1.0);
}
