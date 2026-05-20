#version 450
layout(location=0) in vec2 v_uv;
layout(location=0) out vec4 o_color;
layout(set=0,binding=0) uniform sampler2D u_gbuffer_albedo;
layout(set=0,binding=1) uniform sampler2D u_gbuffer_normal;
layout(set=0,binding=2) uniform sampler2D u_gbuffer_material;
layout(push_constant) uniform DebugGBufferPush { vec4 p[2]; } pc;
void main() {
    int mode = int(pc.p[0].x + 0.5);
    vec3 outc = texture(u_gbuffer_albedo, v_uv).rgb;
    if (mode == 1) outc = texture(u_gbuffer_normal, v_uv).xyz * 0.5 + 0.5;
    if (mode == 2) outc = texture(u_gbuffer_material, v_uv).rgb;
    o_color = vec4(outc, 1.0);
}
