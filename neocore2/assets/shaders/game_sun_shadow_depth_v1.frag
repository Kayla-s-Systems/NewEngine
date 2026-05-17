#version 450
layout(location = 0) in float v_depth;
layout(location = 0) out float o_depth;
void main() {
    o_depth = v_depth;
}
