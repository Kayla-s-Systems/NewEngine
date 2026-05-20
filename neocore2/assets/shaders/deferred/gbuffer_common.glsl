#ifndef NEWENGINE_DEFERRED_GBUFFER_COMMON_GLSL
#define NEWENGINE_DEFERRED_GBUFFER_COMMON_GLSL

vec3 ne_encode_normal(vec3 n) {
    return normalize(n) * 0.5 + 0.5;
}

vec4 ne_pack_gbuffer_albedo(vec3 albedo, float ssao) {
    return vec4(max(albedo, vec3(0.0)), clamp(ssao, 0.0, 1.0));
}

vec4 ne_pack_gbuffer_normal(vec3 world_normal, float twiddle) {
    return vec4(ne_encode_normal(world_normal), clamp(twiddle, 0.0, 1.0));
}

vec4 ne_pack_gbuffer_material(float diffuse_spec_mix, float roughness, float metallic_or_fresnel, float shadow) {
    return vec4(
        clamp(diffuse_spec_mix, 0.0, 1.0),
        clamp(roughness, 0.035, 1.0),
        clamp(metallic_or_fresnel, 0.0, 1.0),
        clamp(shadow, 0.0, 1.0)
    );
}

#endif
