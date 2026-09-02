use super::*;

#[inline]
pub(in super::super) fn sanitize_material_spec(raw: RawMaterialSpec) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: sanitize_asset_path(raw.asset),
        base_color_texture: sanitize_texture_path(raw.base_color_texture),
        normal_texture: sanitize_texture_path(raw.normal_texture),
        roughness_texture: sanitize_texture_path(raw.roughness_texture),
        uv_scale: sanitize_vec2(raw.uv_scale, default_uv_scale()),
        uv_offset: sanitize_vec2(raw.uv_offset, default_uv_offset()),
        roughness: raw.roughness.clamp(0.02, 1.0),
        normal_scale: raw.normal_scale.clamp(0.0, 8.0),
        occlusion_strength: raw.occlusion_strength.clamp(0.0, 1.0),
    }
}

#[inline]
pub(in super::super) fn sanitize_material_spec_with_default_asset(
    raw: RawMaterialSpec,
    fallback: RawMaterialSpec,
) -> GameReadyMaterialSpec {
    let fallback = sanitize_material_spec(fallback);
    let mut spec = sanitize_material_spec(raw);
    if spec.asset.is_none() {
        spec.asset = fallback.asset;
    }
    spec
}

#[inline]
fn sanitize_surface_layer(
    raw: RawTerrainSurfaceLayerSpec,
) -> Option<GameReadyTerrainSurfaceLayerSpec> {
    let role = raw.role.trim().replace('\\', "/");
    let base = if raw.base_texture.trim().is_empty() {
        raw.texture
    } else {
        raw.base_texture
    };
    let base_texture = sanitize_texture_path(Some(base)).unwrap_or_default();
    if role.trim().is_empty() || base_texture.trim().is_empty() {
        return None;
    }
    Some(GameReadyTerrainSurfaceLayerSpec {
        role,
        base_texture,
        weight: raw.weight.clamp(0.0, 8.0),
        uv_scale: raw.uv_scale.clamp(0.0025, 64.0),
    })
}

#[inline]
fn terrain_layer_role_matches(role: &str, aliases: &[&str]) -> bool {
    let role = role.trim();
    aliases.iter().any(|alias| role.eq_ignore_ascii_case(alias))
}

fn canonical_terrain_layer_textures(
    layers: &[GameReadyTerrainSurfaceLayerSpec],
) -> (Option<String>, Option<String>, Option<String>) {
    let mut forest = None;
    let mut sand = None;
    let mut rock = None;
    for layer in layers {
        if forest.is_none()
            && terrain_layer_role_matches(
                &layer.role,
                &["forest", "base", "grass", "meadow", "lowland"],
            )
        {
            forest = Some(layer.base_texture.clone());
        } else if sand.is_none()
            && terrain_layer_role_matches(&layer.role, &["sand", "path", "dirt", "basin", "ground"])
        {
            sand = Some(layer.base_texture.clone());
        } else if rock.is_none()
            && terrain_layer_role_matches(&layer.role, &["rock", "slope", "cliff", "moss"])
        {
            rock = Some(layer.base_texture.clone());
        }
        if forest.is_some() && sand.is_some() && rock.is_some() {
            break;
        }
    }
    (forest, sand, rock)
}

#[inline]
pub(in super::super) fn sanitize_terrain_surface_spec(
    raw: RawTerrainSurfaceSpec,
) -> GameReadyTerrainSurfaceSpec {
    let layers = raw
        .layers
        .into_iter()
        .filter_map(sanitize_surface_layer)
        .collect::<Vec<_>>();
    let (forest_from_layer, sand_from_layer, rock_from_layer) =
        canonical_terrain_layer_textures(&layers);

    GameReadyTerrainSurfaceSpec {
        forest_base_texture: non_empty_or(
            raw.forest_base_texture,
            forest_from_layer.unwrap_or_else(default_terrain_surface_forest),
        ),
        sand_base_texture: non_empty_or(
            raw.sand_base_texture,
            sand_from_layer.unwrap_or_else(default_terrain_surface_sand),
        ),
        rock_base_texture: non_empty_or(
            raw.rock_base_texture,
            rock_from_layer.unwrap_or_else(default_terrain_surface_rock),
        ),
        patch_scale: raw.patch_scale.clamp(0.0025, 0.25),
        blend_softness: raw.blend_softness.clamp(0.01, 0.45),
        layers,
    }
}

#[inline]
pub(in super::super) fn sanitize_terrain_heightmap_spec(
    raw: RawTerrainHeightmapSpec,
) -> GameReadyTerrainHeightmapSpec {
    let source = sanitize_texture_path(Some(raw.source)).unwrap_or_default();
    let mode = match raw.mode.trim().to_ascii_lowercase().as_str() {
        "add" | "additive" => "add",
        "replace" | "override" => "replace",
        _ => "blend",
    }
    .to_owned();
    let mut min_height = if raw.min_height.is_finite() {
        raw.min_height
    } else {
        default_terrain_heightmap_min_height()
    };
    let mut max_height = if raw.max_height.is_finite() {
        raw.max_height
    } else {
        default_terrain_heightmap_max_height()
    };
    if max_height < min_height {
        std::mem::swap(&mut min_height, &mut max_height);
    }
    let mut tile_scale = sanitize_vec2(raw.tile_scale, default_terrain_heightmap_tile_scale());
    tile_scale[0] = tile_scale[0].abs().max(0.0001);
    tile_scale[1] = tile_scale[1].abs().max(0.0001);
    GameReadyTerrainHeightmapSpec {
        enabled: raw.enabled && !source.is_empty(),
        source,
        mode,
        strength: raw.strength.clamp(0.0, 4.0),
        min_height,
        max_height,
        tile_scale,
        tile_offset: sanitize_vec2(raw.tile_offset, default_terrain_heightmap_tile_offset()),
        invert: raw.invert,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_role_matching_is_case_insensitive_without_normalization() {
        assert!(terrain_layer_role_matches(
            "  FoReSt ",
            &["forest", "grass"]
        ));
        assert!(!terrain_layer_role_matches("cliff", &["forest", "grass"]));
    }

    #[test]
    fn canonical_terrain_layers_are_resolved_in_one_pass() {
        let layers = vec![
            GameReadyTerrainSurfaceLayerSpec {
                role: "ground".to_owned(),
                base_texture: "sand".to_owned(),
                weight: 1.0,
                uv_scale: 1.0,
            },
            GameReadyTerrainSurfaceLayerSpec {
                role: "moss".to_owned(),
                base_texture: "rock".to_owned(),
                weight: 1.0,
                uv_scale: 1.0,
            },
            GameReadyTerrainSurfaceLayerSpec {
                role: "meadow".to_owned(),
                base_texture: "forest".to_owned(),
                weight: 1.0,
                uv_scale: 1.0,
            },
        ];
        assert_eq!(
            canonical_terrain_layer_textures(&layers),
            (
                Some("forest".to_owned()),
                Some("sand".to_owned()),
                Some("rock".to_owned())
            )
        );
    }
}
