#version 450

layout(location = 0) in vec2 v_uv;

void main() {
    // Depth-only path: intentionally no color output. Keeping a fragment stage
    // lets alpha-cutout and material-specialized variants share the same pass contract.
}
