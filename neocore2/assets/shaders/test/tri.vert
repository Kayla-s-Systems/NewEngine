#version 450

layout(location = 0) out vec3 vColor;
layout(location = 1) out vec2 vPosNdc;
layout(location = 2) out vec3 vBary;

void main() {
    int i = gl_VertexIndex;

    vec2 p;
    vec3 c;
    vec3 b;

    if (i == 0) {
        p = vec2( 0.0, -0.72);
        c = vec3( 1.0, 0.15, 0.25);
        b = vec3( 1.0, 0.0, 0.0);
    } else if (i == 1) {
        p = vec2( 0.78,  0.68);
        c = vec3( 0.15, 1.0, 0.55);
        b = vec3( 0.0, 1.0, 0.0);
    } else {
        p = vec2(-0.78,  0.68);
        c = vec3( 0.25, 0.55, 1.0);
        b = vec3( 0.0, 0.0, 1.0);
    }

    gl_Position = vec4(p, 0.0, 1.0);
    vColor  = c;
    vPosNdc = p;
    vBary   = b;
}