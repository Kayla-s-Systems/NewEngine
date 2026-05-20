#ifndef NEWENGINE_LIGHT_COMMON_GLSL
#define NEWENGINE_LIGHT_COMMON_GLSL

#define NE_LIGHT_DIRECTIONAL 0u
#define NE_LIGHT_POINT       1u
#define NE_LIGHT_SPOT        2u
#define NE_LIGHT_AREA        3u
#define NE_LIGHT_AMBIENT     4u
#define NE_LIGHT_FLAG_SHADOWED 1u

struct NeLightRecord {
    vec4 pos_radius;       // xyz = view/screen-space proxy position, w = influence radius
    vec4 color_intensity;  // rgb = linear color, a = intensity
    vec4 dir_kind;         // xyz = direction for sun/spot, w = kind as uint-compatible float
    uvec4 flags;           // x = flags, yzw reserved
};

struct NeTileRecord {
    uvec4 offset_count;    // x = index offset, y = count, z = shadowed count, w = reserved
};

struct NeClusterRecord {
    uvec4 offset_count_minmax; // x = index offset, y = count, z/w = min/max depth slice markers
};

layout(push_constant) uniform NeLightGridPush {
    vec4 screen;           // xy = screen size, zw = inverse screen size
    uvec4 grid;            // x = tiles_x, y = tiles_y, z = tile_size_px, w = cluster_z_slices
    uvec4 counts;          // x = light_count, y = max_lights_per_tile, z = tile_count, w = cluster_count
} pc;

uint ne_tile_index(uvec2 tile) {
    return min(tile.y, pc.grid.y - 1u) * pc.grid.x + min(tile.x, pc.grid.x - 1u);
}

uvec2 ne_current_tile() {
    return uvec2(gl_GlobalInvocationID.xy);
}

bool ne_valid_tile(uvec2 tile) {
    return tile.x < pc.grid.x && tile.y < pc.grid.y;
}

uint ne_light_kind(NeLightRecord light) {
    return uint(light.dir_kind.w + 0.5);
}

bool ne_light_shadowed(NeLightRecord light) {
    return (light.flags.x & NE_LIGHT_FLAG_SHADOWED) != 0u;
}

vec2 ne_tile_center_px(uvec2 tile) {
    return (vec2(tile) + vec2(0.5)) * float(pc.grid.z);
}

bool ne_light_intersects_tile(NeLightRecord light, uvec2 tile) {
    uint kind = ne_light_kind(light);
    if (kind == NE_LIGHT_DIRECTIONAL || kind == NE_LIGHT_AMBIENT) {
        return true;
    }
    vec2 center = ne_tile_center_px(tile);
    vec2 delta = center - light.pos_radius.xy;
    float radius = max(light.pos_radius.w, float(pc.grid.z));
    float pad = float(pc.grid.z) * 0.70710678;
    return dot(delta, delta) <= (radius + pad) * (radius + pad);
}

#endif
