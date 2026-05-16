#version 450
layout(location = 0) in float v_depth;
layout(location = 0) out vec4 o_color;
void main() {
    o_color = vec4(v_depth, v_depth, v_depth, 1.0);
}
