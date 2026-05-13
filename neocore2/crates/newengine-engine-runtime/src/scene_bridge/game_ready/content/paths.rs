pub(super) const GAME_READY_APP_DIR: &str = "game-ready-fps";
pub(super) const GAME_READY_PROFILE_FILE: &str = "game_ready_highlands.scene.json";
const PROFILE_ENV_KEYS: [&str; 2] = ["NEWENGINE_GAME_READY_PROFILE", "NEWENGINE_GAME_READY_SCENE"];
/// Logical AssetManager candidates for the game-ready scene profile.
///
/// Environment overrides are treated as logical VFS paths. Absolute filesystem
/// paths are intentionally ignored here: runtime scene text must go through
/// AssetManager/VFS, not direct disk reads.
pub(super) fn profile_asset_candidates() -> Vec<String> {
    let mut out = Vec::new();

    for raw in PROFILE_ENV_KEYS
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let as_path = std::path::Path::new(trimmed);
        if as_path.is_absolute() {
            log::warn!(
                "game-ready: ignoring absolute scene profile path='{}'; set a logical AssetManager path instead",
                trimmed
            );
            continue;
        }
        out.push(normalize_asset_path(trimmed));
    }

    out.push(GAME_READY_PROFILE_FILE.to_owned());
    out.push(format!("scenes/{GAME_READY_PROFILE_FILE}"));
    out.push(format!("game-ready/{GAME_READY_PROFILE_FILE}"));

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
