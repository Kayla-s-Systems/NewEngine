use super::*;

pub(crate) fn profile_path() -> PathBuf {
    newengine_core::config_child("input/bindings.gameplay.json")
}

pub(crate) fn load_profile_from_config(path: &PathBuf) -> Option<InputBindingsProfile> {
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<InputBindingsProfile>(&txt).ok()
}

pub(crate) fn save_profile_to_config(
    path: &PathBuf,
    profile: &InputBindingsProfile,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let txt = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    std::fs::write(path, txt).map_err(|e| e.to_string())
}
