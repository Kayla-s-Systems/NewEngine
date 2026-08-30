mod content_mounts;
mod launch_env;
mod project_browser;
mod project_paths;
mod runtime_profiles;
mod shared_ui;

pub use shared_ui::{
    effective_project_ui_presentation_flow, SHARED_UI_HUD_DOCUMENT_REF, SHARED_UI_HUD_SURFACE_ID,
    SHARED_UI_PAUSE_DOCUMENT_REF, SHARED_UI_PAUSE_SURFACE_ID, SHARED_UI_PRIMARY_TOGGLE_ACTION,
    SHARED_UI_RESUME_ACTION,
};

pub use content_mounts::{mount_content_registry_best_effort, register_engine_asset_roots};
pub use launch_env::*;
pub use project_browser::{
    default_projects_root, discover_game_projects, discover_projects, preferred_game_launch_id,
    preferred_launch_id, ProjectBrowserEntry, ProjectBrowserSelection,
};
pub use project_paths::*;
pub use runtime_profiles::{RuntimeProfileRegistration, RuntimeProfileRegistry};

use std::path::{Path, PathBuf};

use newengine_project_api::{
    ContentMountRegistry, ProjectManifest, ProjectScriptRegistry, ResolvedProjectLaunch,
    RuntimeLaunchProfile, GAME_MANIFEST_ENV, PROJECT_LAUNCH_PRESET_ENV, PROJECT_MANIFEST_FILE,
};

#[derive(Clone, Debug)]
pub struct ProjectRuntimeContext {
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub manifest: ProjectManifest,
    pub launch: ResolvedProjectLaunch,
    pub mounts: ContentMountRegistry,
    pub scripts: ProjectScriptRegistry,
}

impl ProjectRuntimeContext {
    #[inline]
    pub fn paths(&self) -> ProjectPaths {
        ProjectPaths::new(self.project_root.clone())
    }

    #[inline]
    pub fn resolve_authored_path(&self, path: &Path) -> PathBuf {
        self.paths().resolve_authored_path(path)
    }
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

pub fn game_manifest_request_from_environment(
    mut environment_var_os: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
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
    environment_var_os(GAME_MANIFEST_ENV)
        .map(PathBuf::from)
        .or_else(|| project_request_from_environment_lookup(&mut environment_var_os))
        .or_else(adjacent_game_manifest_from_exe)
}

pub fn game_manifest_request_from_process() -> Option<PathBuf> {
    game_manifest_request_from_environment(|name| std::env::var_os(name))
}

pub fn adjacent_game_manifest_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(PROJECT_MANIFEST_FILE);
    candidate.is_file().then_some(candidate)
}

fn project_request_from_environment_lookup(
    environment_var_os: &mut impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
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
    environment_var_os("NEWENGINE_PROJECT").map(PathBuf::from)
}

pub fn project_request_from_environment(
    mut environment_var_os: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    project_request_from_environment_lookup(&mut environment_var_os)
}

pub fn project_request_from_process() -> Option<PathBuf> {
    project_request_from_environment(|name| std::env::var_os(name))
}

pub fn project_launch_request_from_environment(
    mut environment_var: impl FnMut(&str) -> Option<String>,
) -> Option<String> {
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
    environment_var(PROJECT_LAUNCH_PRESET_ENV)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn project_launch_request_from_process() -> Option<String> {
    project_launch_request_from_environment(|name| std::env::var(name).ok())
}

pub fn load_project_from_request(request: &Path) -> Result<ProjectRuntimeContext, String> {
    load_project_from_request_with_launch(request, None)
}

pub fn load_project_from_request_with_launch(
    request: &Path,
    launch_request: Option<&str>,
) -> Result<ProjectRuntimeContext, String> {
    let manifest_path = normalize_project_manifest_request(request.to_path_buf());
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
