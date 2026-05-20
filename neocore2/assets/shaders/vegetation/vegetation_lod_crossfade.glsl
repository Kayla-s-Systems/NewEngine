#ifndef NEWENGINE_VEGETATION_LOD_CROSSFADE_GLSL
#define NEWENGINE_VEGETATION_LOD_CROSSFADE_GLSL

float vegetation_lod_crossfade(float distance_m, float near_end, float far_start, float width_m) {
    float fade_out = 1.0 - smoothstep(near_end - width_m, near_end, distance_m);
    float fade_in = smoothstep(far_start, far_start + width_m, distance_m);
    return clamp(max(fade_out, fade_in), 0.0, 1.0);
}

float vegetation_alpha_dither(float alpha, float fade) {
    float bayer = fract(sin(dot(gl_FragCoord.xy, vec2(41.0, 289.0))) * 951.1357);
    return step(bayer, alpha * fade);
}

#endif
