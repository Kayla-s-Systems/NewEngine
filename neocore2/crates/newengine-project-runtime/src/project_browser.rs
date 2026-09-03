use std::{
    fs,
    path::{Path, PathBuf},
};

use newengine_project_api::{ProjectManifest, RuntimeLaunchProfile, PROJECT_MANIFEST_FILE};

#[derive(Clone, Debug)]
pub struct ProjectBrowserLaunchOption {
    pub id: String,
    pub profile: String,
    pub runtime_profile: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectBrowserEntry {
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub id: String,
    pub name: String,
    pub launcher: Option<String>,
    pub runtime_profile: Option<String>,
    pub launch_profile: Option<String>,
    pub launch_ids: Vec<String>,
    pub launch_options: Vec<ProjectBrowserLaunchOption>,
    pub default_launch: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectBrowserSelection {
    pub manifest_path: Option<PathBuf>,
    pub launch_id: Option<String>,
    pub cancelled: bool,
}

pub fn default_projects_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NEWENGINE_PROJECTS_ROOT") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }

    let mut seeds = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            seeds.push(parent.to_path_buf());
        }
    }
    for seed in seeds {
        for ancestor in seed.ancestors().take(8) {
            let candidate = ancestor.join("Projects");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn discover_projects(root: &Path) -> Vec<ProjectBrowserEntry> {
    let mut out = Vec::new();
    discover_recursive(root, 0, &mut out);
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

pub fn discover_game_projects(root: &Path) -> Vec<ProjectBrowserEntry> {
    discover_projects(root)
        .into_iter()
        .filter_map(|mut entry| {
            let source = fs::read_to_string(&entry.manifest_path).ok()?;
            let manifest = toml::from_str::<ProjectManifest>(&source).ok()?;
            let launch_id = preferred_game_launch_id(&manifest)?;
            entry.launch_profile = Some(RuntimeLaunchProfile::Game.id().to_owned());
            entry.launch_ids = vec![launch_id.clone()];
            entry.launch_options =
                manifest
                    .resolve_launch(Some(&launch_id))
                    .ok()
                    .map(|resolved| {
                        vec![ProjectBrowserLaunchOption {
                            id: launch_id.clone(),
                            profile: resolved.profile.id().to_owned(),
                            runtime_profile: resolved.runtime_profile,
                        }]
                    })?;
            entry.default_launch = launch_id;
            Some(entry)
        })
        .collect()
}

pub fn preferred_launch_id(manifest: &ProjectManifest) -> String {
    if let Some(default_launch) = manifest
        .default_launch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return default_launch.to_owned();
    }
    if let Some(id) = manifest.launch_ids().into_iter().next() {
        return id;
    }
    manifest
        .launch_profile
        .map(|profile| profile.id().to_owned())
        .unwrap_or_else(|| "game".to_owned())
}

pub fn preferred_game_launch_id(manifest: &ProjectManifest) -> Option<String> {
    let profile_for = |id: &str| {
        manifest
            .launch
            .get(id)
            .and_then(|preset| preset.profile)
            .or(manifest.launch_profile)
            .unwrap_or_default()
    };

    if manifest.launch.contains_key("game") && profile_for("game") == RuntimeLaunchProfile::Game {
        return Some("game".to_owned());
    }
    if let Some(default_launch) = manifest
        .default_launch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if manifest.launch.contains_key(default_launch)
            && profile_for(default_launch) == RuntimeLaunchProfile::Game
        {
            return Some(default_launch.to_owned());
        }
    }
    if let Some((id, _)) = manifest
        .launch
        .iter()
        .find(|(id, _)| profile_for(id) == RuntimeLaunchProfile::Game)
    {
        return Some(id.clone());
    }
    (manifest.launch_profile.unwrap_or_default() == RuntimeLaunchProfile::Game)
        .then(|| "game".to_owned())
}

fn discover_recursive(dir: &Path, depth: usize, out: &mut Vec<ProjectBrowserEntry>) {
    if depth > 6 {
        return;
    }
    let manifest_path = dir.join(PROJECT_MANIFEST_FILE);
    if manifest_path.is_file() {
        if let Ok(text) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = toml::from_str::<ProjectManifest>(&text) {
                if manifest.validate().is_ok() {
                    let default_launch = manifest
                        .default_launch
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .or_else(|| {
                            manifest
                                .launch_profile
                                .map(|profile| profile.id().to_owned())
                        })
                        .unwrap_or_else(|| "game".to_owned());
                    let mut launch_ids = manifest.launch_ids();
                    if !launch_ids.iter().any(|id| id == &default_launch) {
                        launch_ids.push(default_launch.clone());
                    }
                    let launch_options = launch_ids
                        .iter()
                        .filter_map(|id| {
                            manifest.resolve_launch(Some(id)).ok().map(|resolved| {
                                ProjectBrowserLaunchOption {
                                    id: id.clone(),
                                    profile: resolved.profile.id().to_owned(),
                                    runtime_profile: resolved.runtime_profile,
                                }
                            })
                        })
                        .collect();
                    out.push(ProjectBrowserEntry {
                        manifest_path,
                        project_root: dir.to_path_buf(),
                        id: manifest.id.clone(),
                        name: if manifest.name.trim().is_empty() {
                            manifest.id
                        } else {
                            manifest.name
                        },
                        launcher: manifest.launcher,
                        runtime_profile: manifest.runtime_profile,
                        launch_profile: manifest
                            .launch_profile
                            .map(|profile| profile.id().to_owned()),
                        launch_ids,
                        launch_options,
                        default_launch,
                    });
                    return;
                }
            }
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                ".git" | "target" | "intermediate" | "node_modules"
            ) {
                continue;
            }
            discover_recursive(&path, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_root_discovery_is_bounded() {
        if let Some(root) = default_projects_root() {
            assert!(root.ends_with("Projects"));
        }
    }

    #[test]
    fn game_launch_selector_prefers_declared_game_preset() {
        let mut manifest = ProjectManifest {
            launch_profile: Some(RuntimeLaunchProfile::Game),
            default_launch: Some("editor".to_owned()),
            ..ProjectManifest::default()
        };
        manifest.launch.insert(
            "game".to_owned(),
            newengine_project_api::ProjectLaunchPreset {
                profile: Some(RuntimeLaunchProfile::Game),
                ..Default::default()
            },
        );
        assert_eq!(preferred_game_launch_id(&manifest).as_deref(), Some("game"));
    }
}
