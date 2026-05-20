#version 450

layout(set = 0, binding = 0) uniform sampler2D u_gbuffer_albedo;   // RGB albedo, A screen-space ambient/occlusion
layout(set = 0, binding = 1) uniform sampler2D u_gbuffer_normal;   // RGB encoded world normal, A normal twiddle/validity
layout(set = 0, binding = 2) uniform sampler2D u_gbuffer_material; // R diffuse/spec mix, G roughness, B metallic/fresnel, A shadow/occlusion
layout(set = 0, binding = 3) uniform sampler2D u_gbuffer_depth;

#define NE_LIGHT_DIRECTIONAL 0u
#define NE_LIGHT_POINT       1u
#define NE_LIGHT_SPOT        2u
#define NE_LIGHT_AREA        3u
#define NE_LIGHT_AMBIENT     4u
#define NE_LIGHT_FLAG_SHADOWED 1u

struct NeLightRecord {
    vec4 pos_radius;
    vec4 color_intensity;
    vec4 dir_kind;
    uvec4 flags;
};

struct NeTileRecord {
    uvec4 offset_count;
};

struct NeClusterRecord {
    uvec4 offset_count_minmax;
};

layout(set = 0, binding = 4, std430) readonly buffer LightBuffer {
    NeLightRecord lights[];
} u_lights;

layout(set = 0, binding = 5, std430) readonly buffer TileGridBuffer {
    NeTileRecord tiles[];
} u_tiles;

layout(set = 0, binding = 6, std430) readonly buffer TileLightIndexBuffer {
    uint indices[];
} u_indices;

layout(set = 0, binding = 7, std430) readonly buffer ClusterGridBuffer {
    NeClusterRecord clusters[];
} u_clusters;

layout(push_constant) uniform DeferredLightingPush {
    vec4 screen;                    // xy = extent, zw = inverse extent
    vec4 ambient;                   // rgb = ambient color, a = ambient intensity
    vec4 light_direction_intensity; // xyz = world light direction, w = intensity
    vec4 light_color;               // rgb = directional color
    vec4 tile_info;                 // x = tile size, y = tiles x, z = tiles y, w = cluster z slices
} pc;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

vec3 decode_normal(vec4 packed) {
    vec3 n = packed.xyz * 2.0 - 1.0;
    float len2 = max(dot(n, n), 1.0e-5);
    return n * inversesqrt(len2);
}

uint tile_index_from_frag() {
    vec2 px = gl_FragCoord.xy;
    uint tile_size = max(uint(pc.tile_info.x + 0.5), 1u);
    uint tiles_x = max(uint(pc.tile_info.y + 0.5), 1u);
    uint tiles_y = max(uint(pc.tile_info.z + 0.5), 1u);
    uvec2 tile = uvec2(px) / tile_size;
    tile = min(tile, uvec2(tiles_x - 1u, tiles_y - 1u));
    return tile.y * tiles_x + tile.x;
}

vec3 local_light_accum(uint tile_index, vec3 albedo, vec3 normal_ws, float roughness, float metallic, float shadow, float depth) {
    NeTileRecord tile = u_tiles.tiles[tile_index];
    uint offset = tile.offset_count.x;
    uint count = min(tile.offset_count.y, 128u);
    vec2 frag_px = gl_FragCoord.xy;
    vec3 color = vec3(0.0);

    for (uint i = 0u; i < count; ++i) {
        uint light_index = u_indices.indices[offset + i];
        NeLightRecord light = u_lights.lights[light_index];
        uint kind = uint(light.dir_kind.w + 0.5);

        if (kind == NE_LIGHT_DIRECTIONAL) {
            vec3 l = normalize(-light.dir_kind.xyz);
            float ndotl = max(dot(normal_ws, l), 0.0);
            float shadow_term = ((light.flags.x & NE_LIGHT_FLAG_SHADOWED) != 0u) ? shadow : 1.0;
            color += albedo * light.color_intensity.rgb * light.color_intensity.a * ndotl * shadow_term;
            continue;
        }

        if (kind == NE_LIGHT_POINT || kind == NE_LIGHT_SPOT) {
            vec2 delta = light.pos_radius.xy - frag_px;
            float radius = max(light.pos_radius.w, 1.0);
            float dist2 = dot(delta, delta);
            float attenuation = clamp(1.0 - dist2 / (radius * radius), 0.0, 1.0);
            attenuation *= attenuation;
            float normal_bias = 0.35 + 0.65 * max(normal_ws.z, 0.0);
            float shadow_term = ((light.flags.x & NE_LIGHT_FLAG_SHADOWED) != 0u) ? mix(1.0, shadow, 0.65) : 1.0;
            vec3 f0 = mix(vec3(0.04), albedo, metallic);
            float spec = pow(max(normal_ws.z, 0.0), mix(96.0, 8.0, roughness)) * (1.0 - roughness);
            color += (albedo * normal_bias + f0 * spec) * light.color_intensity.rgb * light.color_intensity.a * attenuation * shadow_term;
        }
    }

    // Slightly fade local light with scene depth until linear depth reconstruction lands.
    return color * smoothstep(1.0, 0.15, depth);
}

void main() {
    vec2 uv = clamp(v_uv, vec2(0.0), vec2(1.0));
    vec4 albedo_occ = texture(u_gbuffer_albedo, uv);
    vec4 normal_twiddle = texture(u_gbuffer_normal, uv);
    vec4 material_shadow = texture(u_gbuffer_material, uv);
    float depth = texture(u_gbuffer_depth, uv).r;

    vec3 albedo = max(albedo_occ.rgb, vec3(0.0));
    float ssao = clamp(albedo_occ.a, 0.0, 1.0);
    vec3 n = decode_normal(normal_twiddle);

    float diffuse_spec_mix = clamp(material_shadow.r, 0.0, 1.0);
    float roughness = clamp(material_shadow.g, 0.035, 1.0);
    float metallic = clamp(material_shadow.b, 0.0, 1.0);
    float shadow = clamp(material_shadow.a, 0.0, 1.0);

    vec3 light_dir = normalize(-pc.light_direction_intensity.xyz);
    vec3 view_dir = vec3(0.0, 0.0, 1.0);
    vec3 half_dir = normalize(light_dir + view_dir);

    float ndotl = max(dot(n, light_dir), 0.0);
    float spec_power = mix(96.0, 8.0, roughness);
    float spec = pow(max(dot(n, half_dir), 0.0), spec_power) * (1.0 - roughness);

    vec3 f0 = mix(vec3(0.04), albedo, metallic);
    float fresnel = pow(1.0 - max(dot(view_dir, half_dir), 0.0), 5.0);
    vec3 spec_color = mix(f0, vec3(1.0), fresnel);

    vec3 ambient = albedo * pc.ambient.rgb * pc.ambient.a * ssao;
    vec3 diffuse = albedo * pc.light_color.rgb * pc.light_direction_intensity.w * ndotl * shadow;
    vec3 highlight = spec_color * spec * pc.light_color.rgb * diffuse_spec_mix * shadow;
    vec3 local_lighting = local_light_accum(tile_index_from_frag(), albedo, n, roughness, metallic, shadow, depth);

    float far_fade = smoothstep(1.0, 0.92, depth);
    vec3 color = (ambient + diffuse + highlight + local_lighting) * far_fade;
    o_color = vec4(color, 1.0);
}
