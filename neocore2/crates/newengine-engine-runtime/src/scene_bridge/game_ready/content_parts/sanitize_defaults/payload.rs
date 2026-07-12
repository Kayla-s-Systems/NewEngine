use super::*;

// Strict data-driven mode: authored .ymap is required; no emergency runtime profile is generated.
impl Default for RawGameReadyPayload {
    fn default() -> Self {
        Self {
            title: default_title(),
            objective: default_objective(),
            player: RawPlayerSpec::default(),
            terrain: RawTerrainSpec::default(),
            sky: RawSkySpec::default(),
            materials: RawMaterialSetSpec::default(),
            lighting: RawLightingSpec::default(),
            foliage: RawFoliageSpec::default(),
            prefabs: Vec::new(),
            definitions: Vec::new(),
            gameplay: RawGameplaySpec::default(),
            palette: RawPaletteSpec::default(),
        }
    }
}

#[inline]
pub(in super::super) fn non_empty_or(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else if trimmed.len() == value.len() {
        value
    } else {
        trimmed.to_owned()
    }
}

#[inline]
pub(in super::super) fn sanitize_texture_path(value: Option<String>) -> Option<String> {
    sanitize_asset_path(value)
}

#[inline]
pub(in super::super) fn sanitize_asset_path(value: Option<String>) -> Option<String> {
    value.and_then(|path| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.len() == path.len() && !path.contains('\\') {
            return Some(path);
        }
        Some(trimmed.replace('\\', "/"))
    })
}

#[inline]
pub(in super::super) fn sanitize_vec2(mut v: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    for i in 0..2 {
        if !v[i].is_finite() || v[i].abs() <= 1.0e-6 {
            v[i] = fallback[i];
        }
    }
    v
}
