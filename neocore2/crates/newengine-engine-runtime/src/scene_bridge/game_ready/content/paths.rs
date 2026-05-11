use std::path::{Path, PathBuf};

const GAME_READY_APP_DIR: &str = "game-ready-fps";
const GAME_READY_PROFILE_FILE: &str = "game_ready_highlands.scene.json";
const PROFILE_ENV_KEYS: [&str; 2] = ["NEWENGINE_GAME_READY_PROFILE", "NEWENGINE_GAME_READY_SCENE"];
const PLUGIN_DIR_ENV_KEYS: [&str; 3] = [
    "NEWENGINE_PLUGIN_DIR",
    "NEWENGINE_PLUGINS_DIR",
    "NEWENGINE_MODULES_DIR",
];

pub(super) fn profile_file_candidates() -> Vec<PathBuf> {
    let mut out = env_path_candidates(PROFILE_ENV_KEYS);

    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("assets").join(GAME_READY_PROFILE_FILE));
        out.push(app_profile_path(&cwd));
        out.push(app_profile_path(&cwd.join("NewEngine").join("neocore2")));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent() {
            out.push(debug_dir.join("assets").join(GAME_READY_PROFILE_FILE));
            if let Some(workspace_dir) = debug_dir
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
            {
                out.push(app_profile_path(workspace_dir));
            }
        }
    }

    dedup_paths(out)
}

pub(super) fn plugin_dir_candidates() -> Vec<PathBuf> {
    let mut out = env_path_candidates(PLUGIN_DIR_ENV_KEYS);

    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("plugins"));
        out.push(cwd.join("NewEngine").join("neocore2").join("plugins"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.to_path_buf());
            out.push(parent.join("plugins"));
            if let Some(profile) = parent.parent() {
                out.push(profile.join("plugins"));
                if let Some(target) = profile.parent() {
                    out.push(target.join("plugins"));
                }
            }
        }
    }

    dedup_paths(out)
}

fn env_path_candidates<const N: usize>(keys: [&str; N]) -> Vec<PathBuf> {
    keys.into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn app_profile_path(root: &Path) -> PathBuf {
    root.join("apps")
        .join(GAME_READY_APP_DIR)
        .join("assets")
        .join(GAME_READY_PROFILE_FILE)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }
    out
}
