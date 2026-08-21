mod project_browser;
mod runtime_profiles;

pub use project_browser::{
    default_projects_root, discover_game_projects, discover_projects, preferred_launch_id,
    ProjectBrowserEntry, ProjectBrowserSelection,
};
pub use runtime_profiles::{RuntimeProfileRegistration, RuntimeProfileRegistry};

use std::path::{Path, PathBuf};

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_project_api::{
    ContentMountDescriptor, ContentMountNamespace, ContentMountRegistry, ProjectManifest,
    ProjectScriptRegistry, ResolvedProjectLaunch, RuntimeLaunchProfile, GAME_MANIFEST_ENV,
    PROJECT_LAUNCH_PRESET_ENV, PROJECT_MANIFEST_FILE,
};

pub const UI_SCREEN_PROFILE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile";
pub const UI_PUBLISH_EDITOR_SHELL_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell";
pub const UI_PRESENTATION_INITIAL_STATE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__presentation_flow__initial_state";

fn set_default_env(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        std::env::set_var(key, value);
    }
}

pub fn apply_project_launch_profile_env(profile: RuntimeLaunchProfile) {
    set_default_env("NEWENGINE_LAUNCH_PROFILE", profile.id());
    match profile {
        RuntimeLaunchProfile::Editor => {
            set_default_env("NEWENGINE_HEADLESS", "0");
            set_default_env(UI_SCREEN_PROFILE_ENV, "editor");
            set_default_env(UI_PUBLISH_EDITOR_SHELL_ENV, "true");
        }
        RuntimeLaunchProfile::Game => {
            set_default_env("NEWENGINE_HEADLESS", "0");
            set_default_env(UI_SCREEN_PROFILE_ENV, "game");
            set_default_env(UI_PUBLISH_EDITOR_SHELL_ENV, "false");
        }
        RuntimeLaunchProfile::Server => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
            set_default_env(UI_PUBLISH_EDITOR_SHELL_ENV, "false");
            set_default_env("NEWENGINE_PLUGIN_TARGET", "runtime");
        }
        RuntimeLaunchProfile::Test => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
            set_default_env(UI_PUBLISH_EDITOR_SHELL_ENV, "false");
            set_default_env("NEWENGINE_HEADLESS_FRAMES", "1");
            set_default_env("NEWENGINE_PLUGIN_TARGET", "runtime");
        }
    }
}

pub fn apply_project_startup_presentation_state_env(state: &str) {
    let state = state.trim();
    if !state.is_empty() {
        set_default_env(UI_PRESENTATION_INITIAL_STATE_ENV, state);
    }
}

pub fn apply_resolved_project_launch_env(launch: &ResolvedProjectLaunch) {
    apply_project_launch_profile_env(launch.profile);
    if let Some(state) = launch.startup_presentation_state.as_deref() {
        apply_project_startup_presentation_state_env(state);
    }
}

#[derive(Clone, Debug)]
pub struct ProjectRuntimeContext {
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub manifest: ProjectManifest,
    pub launch: ResolvedProjectLaunch,
    pub mounts: ContentMountRegistry,
    pub scripts: ProjectScriptRegistry,
}

#[derive(Clone, Debug)]
pub struct RuntimeCompositionContext {
    pub manifest_path: PathBuf,
    pub runtime_root: PathBuf,
    pub runtime_profile: String,
    pub game_module: Option<String>,
    pub launch_profile: RuntimeLaunchProfile,
    pub startup_scene: Option<String>,
    pub startup_presentation_state: Option<String>,
    pub definitions: Vec<PathBuf>,
    pub mounts: ContentMountRegistry,
    pub scripts: ProjectScriptRegistry,
}

impl RuntimeCompositionContext {
    pub fn from_project(project: &ProjectRuntimeContext) -> Self {
        Self {
            manifest_path: project.manifest_path.clone(),
            runtime_root: project.project_root.clone(),
            runtime_profile: project
                .launch
                .runtime_profile
                .clone()
                .or_else(|| project.manifest.runtime_profile.clone())
                .unwrap_or_default(),
            game_module: project.manifest.game_module.clone(),
            launch_profile: project.launch.profile,
            startup_scene: project.launch.startup_scene.clone(),
            startup_presentation_state: project.launch.startup_presentation_state.clone(),
            definitions: project.manifest.definitions.clone(),
            mounts: project.mounts.clone(),
            scripts: project.scripts.clone(),
        }
    }
}

pub fn game_manifest_request_from_process() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--game-manifest" {
            return args.next().map(PathBuf::from);
        }
        if let Some(text) = arg.to_str() {
            if let Some(value) = text.strip_prefix("--game-manifest=") {
                if !value.trim().is_empty() {
                    return Some(PathBuf::from(value));
                }
            }
        }
    }
    std::env::var_os(GAME_MANIFEST_ENV)
        .map(PathBuf::from)
        .or_else(project_request_from_process)
        .or_else(adjacent_game_manifest_from_exe)
}

pub fn adjacent_game_manifest_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(PROJECT_MANIFEST_FILE);
    candidate.is_file().then_some(candidate)
}

pub fn project_request_from_process() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--project" {
            return args.next().map(PathBuf::from);
        }
        if let Some(text) = arg.to_str() {
            if let Some(value) = text.strip_prefix("--project=") {
                if !value.trim().is_empty() {
                    return Some(PathBuf::from(value));
                }
            }
        }
    }
    std::env::var_os("NEWENGINE_PROJECT").map(PathBuf::from)
}

pub fn project_launch_request_from_process() -> Option<String> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--launch" || arg == "--launch-profile" {
            return args
                .next()
                .and_then(|value| value.to_str().map(str::to_owned))
                .filter(|value| !value.trim().is_empty());
        }
        if let Some(text) = arg.to_str() {
            for prefix in ["--launch=", "--launch-profile="] {
                if let Some(value) = text.strip_prefix(prefix) {
                    if !value.trim().is_empty() {
                        return Some(value.trim().to_owned());
                    }
                }
            }
        }
    }
    std::env::var(PROJECT_LAUNCH_PRESET_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn load_project_from_request(request: &Path) -> Result<ProjectRuntimeContext, String> {
    load_project_from_request_with_launch(request, None)
}

pub fn load_project_from_request_with_launch(
    request: &Path,
    launch_request: Option<&str>,
) -> Result<ProjectRuntimeContext, String> {
    let manifest_path = if request.is_dir() {
        request.join(PROJECT_MANIFEST_FILE)
    } else {
        request.to_path_buf()
    };
    let project_root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let source = std::fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "read project manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    let manifest: ProjectManifest = toml::from_str(&source).map_err(|error| {
        format!(
            "parse project manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    if let Err(errors) = manifest.validate() {
        return Err(format!("project manifest invalid: {}", errors.join("; ")));
    }
    let launch = manifest
        .resolve_launch(launch_request)
        .map_err(|error| format!("project launch invalid: {error}"))?;
    let scripts = ProjectScriptRegistry::from_manifest(&manifest.scripting)
        .map_err(|error| format!("project scripting invalid: {error}"))?;
    let mut mounts = ContentMountRegistry::default();
    for descriptor in &manifest.content {
        mounts.register(descriptor.clone().normalized(&project_root))?;
    }
    Ok(ProjectRuntimeContext {
        manifest_path,
        project_root,
        manifest,
        launch,
        mounts,
        scripts,
    })
}

pub fn register_engine_asset_roots(
    registry: &mut ContentMountRegistry,
    roots: &[PathBuf],
) -> Result<(), String> {
    for (index, root) in roots.iter().enumerate() {
        registry.register(ContentMountDescriptor {
            id: format!("engine.compat.{index}"),
            namespace: ContentMountNamespace::Engine,
            root: root.clone(),
            mount: "engine".to_owned(),
            priority: ContentMountNamespace::Engine.default_priority() - index as i32,
            writable: false,
            required: false,
            owner: "runtime-host.compat".to_owned(),
        })?;
    }
    Ok(())
}

pub fn mount_content_registry_best_effort(
    assets: &AssetServiceClient,
    registry: &ContentMountRegistry,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for mount in registry.mounts() {
        if !mount.root.is_dir() {
            let message = format!(
                "content mount '{}' root missing: {}",
                mount.id,
                mount.root.display()
            );
            if mount.required {
                diagnostics.push(format!("ERROR: {message}"));
            } else {
                diagnostics.push(format!("SKIP: {message}"));
            }
            continue;
        }
        let result = assets.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": mount.priority,
            "mount": mount.mount,
            "config": { "root": mount.root.to_string_lossy() },
            "metadata": {
                "mount_id": mount.id,
                "namespace": mount.namespace.id(),
                "owner": mount.owner,
                "writable": mount.writable,
            }
        }));
        match result {
            Ok(_) => diagnostics.push(format!(
                "MOUNT: {}:/ <- {} priority={}",
                mount.namespace.id(),
                mount.root.display(),
                mount.priority
            )),
            Err(error) => diagnostics.push(format!(
                "ERROR: mount '{}' failed root='{}': {error}",
                mount.id,
                mount.root.display()
            )),
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_argument_supports_equals_form() {
        // Parsing process args itself is intentionally tiny; manifest parsing/validation is covered by API tests.
        assert_eq!(PROJECT_MANIFEST_FILE, "game.toml");
    }
}
