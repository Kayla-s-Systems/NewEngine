#ifndef NEWENGINE_VISIBILITY_COMMON_GLSL
#define NEWENGINE_VISIBILITY_COMMON_GLSL

layout(push_constant) uniform VisibilityPush {
    uvec4 screen; // width, height, hiz mip count, screen tiles
    uvec4 counts; // queries, zones, pvs sectors, phase masks
    uvec4 flags;  // hiz ready, async query, pvs sort, zone attributes
    uvec4 frame;  // frame low/high
} pc;

struct NeVisibilityFeedback {
    uvec4 visibility; // visible bit, confidence, pixel count, phase mask
};

struct NePvsRecord {
    uvec4 sector_key_distance; // sector, packed distance, visibility, zone mask
};

struct NeZoneRecord {
    uvec4 flags_mask_count; // zone flags, phase mask, visible count, reserved
};

uint ne_tile_index() {
    return gl_GlobalInvocationID.y * max(1u, gl_NumWorkGroups.x) + gl_GlobalInvocationID.x;
}

bool ne_valid_tile() {
    return gl_GlobalInvocationID.x < max(1u, gl_NumWorkGroups.x)
        && gl_GlobalInvocationID.y < max(1u, gl_NumWorkGroups.y);
}

uint ne_hash_u32(uint x) {
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

#endif
