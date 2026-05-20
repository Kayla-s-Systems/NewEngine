#version 450
#extension GL_GOOGLE_include_directive : require
#include "csm_shadow_common.glsl"

layout(set = 1, binding = 0) uniform sampler2D u_alpha_mask;
layout(location = 0) in vec2 v_uv;

void main() {
    float alpha_ref = csm.depth_bias_normal_bias_alpha_ref_flags.z;
    if (texture(u_alpha_mask, v_uv).a < alpha_ref) {
        discard;
    }
}
