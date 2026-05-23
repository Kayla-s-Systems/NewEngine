pub(super) const SCENE_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";
const PROFILE_ENV_KEYS: [&str; 1] = [SCENE_PROFILE_ENV];
/// Logical AssetManager candidates for the game-ready authored map.
///
/// Environment overrides are treated as logical VFS paths. Absolute filesystem
/// paths are intentionally ignored here: authored map data must go through
/// AssetManager/VFS and the NEF8/ListFile codec, not direct disk reads.
pub(super) fn profile_asset_candidates() -> Vec<String> {
    let mut out = Vec::new();

    for raw in PROFILE_ENV_KEYS
        .into_iter()
        .filter_map(|key| crate::env_config::var(key))
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let as_path = std::path::Path::new(trimmed);
        if as_path.is_absolute() {
            log::warn!(
                "scene profile: ignoring absolute authored map path='{}'; set a logical AssetManager path instead",
                trimmed
            );
            continue;
        }
        out.push(normalize_asset_path(trimmed));
    }

    if out.is_empty() {
        log::warn!(
            "scene profile: no authored map profile configured; set NEWENGINE_SCENE_PROFILE to a logical .ymap path from the active profile layer"
        );
    }

    dedup_strings(out)
}

fn normalize_asset_path(path: &str) -> String {
    path.trim().trim_start_matches('/').replace('\\', "/")
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        if !out.iter().any(|existing| existing == &value) {
            out.push(value);
        }
    }
    out
}
