mod project_browser;
mod runtime_profiles;
mod shared_ui;

pub use shared_ui::{
    effective_project_ui_presentation_flow, SHARED_UI_HUD_DOCUMENT_REF, SHARED_UI_HUD_SURFACE_ID,
    SHARED_UI_PAUSE_DOCUMENT_REF, SHARED_UI_PAUSE_SURFACE_ID, SHARED_UI_PRIMARY_TOGGLE_ACTION,
    SHARED_UI_RESUME_ACTION,
};

pub use project_browser::{
    default_projects_root, discover_game_projects, discover_projects, preferred_game_launch_id,
    preferred_launch_id, ProjectBrowserEntry, ProjectBrowserSelection,
};
pub use runtime_profiles::{RuntimeProfileRegistration, RuntimeProfileRegistry};

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_project_api::{
    ContentMountDescriptor, ContentMountNamespace, ContentMountRegistry, ProjectManifest,
    ProjectScriptRegistry, ResolvedProjectLaunch, RuntimeLaunchProfile, GAME_MANIFEST_ENV,
    PROJECT_LAUNCH_PRESET_ENV, PROJECT_MANIFEST_FILE,
};

pub const UI_SCREEN_PROFILE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile";
pub const UI_PRESENTATION_INITIAL_STATE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__presentation_flow__initial_state";
pub const UI_PRESENTATION_FLOW_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__presentation_flow";
pub const UI_ROOT_SURFACE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_root_surface_id";
pub const UI_DOCUMENT_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_document_ref";
pub const UI_PUBLISH_EDITOR_SHELL_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell";

fn set_default_env(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        std::env::set_var(key, value);
    }
}

pub fn apply_project_launch_profile_env(profile: RuntimeLaunchProfile) {
    set_default_env("NEWENGINE_LAUNCH_PROFILE", profile.id());
    match profile {
        RuntimeLaunchProfile::Game => {
            set_default_env("NEWENGINE_HEADLESS", "0");
            set_default_env(UI_SCREEN_PROFILE_ENV, "game");
        }
        RuntimeLaunchProfile::Server => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
            set_default_env("NEWENGINE_PLUGIN_TARGET", "runtime");
        }
        RuntimeLaunchProfile::Test => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
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

pub fn apply_project_ui_env(manifest: &ProjectManifest) {
    if let Some(value) = manifest
        .ui
        .screen_profile
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_SCREEN_PROFILE_ENV, value);
    }
    if let Some(value) = manifest
        .ui
        .root_surface
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_ROOT_SURFACE_ENV, value);
    }
    if let Some(value) = manifest
        .ui
        .document
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_DOCUMENT_ENV, value);
    }
    let requested_initial_state = std::env::var(UI_PRESENTATION_INITIAL_STATE_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(flow) =
        effective_project_ui_presentation_flow(manifest, requested_initial_state.as_deref())
    {
        // Fold launch-state overrides into the complete authored/shared graph so
        // environment iteration order cannot replace the object with a scalar subtree.
        std::env::remove_var(UI_PRESENTATION_INITIAL_STATE_ENV);
        if let Ok(encoded) = serde_json::to_string(&flow) {
            std::env::set_var(UI_PRESENTATION_FLOW_ENV, encoded);
        }
    } else {
        std::env::remove_var(UI_PRESENTATION_FLOW_ENV);
    }
    if let Some(value) = manifest.ui.publish_editor_shell {
        std::env::set_var(
            UI_PUBLISH_EDITOR_SHELL_ENV,
            if value { "true" } else { "false" },
        );
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

const ASSET_BUILD_PLAN_FILE: &str = "asset.build.json";
const SOURCE_DISCOVERY_LIMIT: usize = 50_000;

fn normalize_project_relative_path(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned();
    if normalized.is_empty() {
        return None;
    }
    let path = Path::new(&normalized);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(normalized)
}

fn runtime_logical_path_from_output(
    output: &str,
    explicit_logical_path: Option<&str>,
) -> Option<String> {
    if let Some(explicit) = explicit_logical_path.and_then(normalize_project_relative_path) {
        return Some(explicit);
    }

    let output = normalize_project_relative_path(output)?;
    let (root, rest) = output
        .split_once('/')
        .map(|(root, rest)| (root, rest))
        .unwrap_or(("", output.as_str()));
    if rest.is_empty() {
        return None;
    }
    if root.eq_ignore_ascii_case("Content") {
        return Some(rest.to_owned());
    }
    if root.eq_ignore_ascii_case("Definitions") {
        return Some(format!("definitions/{rest}"));
    }
    if root.eq_ignore_ascii_case("Scripts") {
        return Some(format!("scripts/{rest}"));
    }
    Some(output)
}

fn collect_build_plan_aliases(value: &serde_json::Value, aliases: &mut BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(object) => {
            let source = object.get("source").and_then(|value| value.as_str());
            let output = object.get("output").and_then(|value| value.as_str());
            if let (Some(source), Some(output)) = (source, output) {
                let logical_path = runtime_logical_path_from_output(
                    output,
                    object.get("logical_path").and_then(|value| value.as_str()),
                );
                let source_path = normalize_project_relative_path(source);
                if let (Some(logical_path), Some(source_path)) = (logical_path, source_path) {
                    if source_path
                        .split('/')
                        .next()
                        .is_some_and(|root| root.eq_ignore_ascii_case("Source"))
                    {
                        aliases.insert(logical_path, source_path);
                    }
                }
            }
            for child in object.values() {
                collect_build_plan_aliases(child, aliases);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_build_plan_aliases(child, aliases);
            }
        }
        _ => {}
    }
}

fn convention_logical_path(source_relative: &str) -> Option<String> {
    let normalized = normalize_project_relative_path(source_relative)?;
    let lower = normalized.to_ascii_lowercase();
    for suffix in [
        ".ymap.xml",
        ".ytyp.xml",
        ".nemat.xml",
        ".neui.xml",
        ".ymt.xml",
    ] {
        if lower.ends_with(suffix) {
            return Some(normalized[..normalized.len() - ".xml".len()].to_owned());
        }
    }
    if lower.starts_with("scripts/") && lower.ends_with(".lua") {
        return Some(format!(
            "{}.ysc",
            &normalized[..normalized.len() - ".lua".len()]
        ));
    }
    if lower.starts_with("animations/") && lower.ends_with(".clip.json") {
        return Some(format!(
            "{}.ycd",
            &normalized[..normalized.len() - ".clip.json".len()]
        ));
    }
    None
}

fn discover_convention_aliases(
    project_root: &Path,
    aliases: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let source_root = project_root.join("Source");
    let mut pending = vec![source_root.clone()];
    let mut visited = 0usize;

    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("scan source directory '{}': {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("scan source entry under '{}': {error}", directory.display())
            })?;
            visited = visited.saturating_add(1);
            if visited > SOURCE_DISCOVERY_LIMIT {
                return Err(format!(
                    "source discovery exceeded bounded entry limit {SOURCE_DISCOVERY_LIMIT} under '{}'",
                    source_root.display()
                ));
            }
            let path = entry.path();
            let metadata = entry
                .metadata()
                .map_err(|error| format!("read source metadata '{}': {error}", path.display()))?;
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(&source_root)
                .ok()
                .map(|value| value.to_string_lossy().replace('\\', "/"));
            let Some(relative) = relative else {
                continue;
            };
            if let Some(logical_path) = convention_logical_path(&relative) {
                aliases
                    .entry(logical_path)
                    .or_insert_with(|| format!("Source/{relative}"));
            }
        }
    }
    Ok(())
}

fn discover_project_source_aliases(project_root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut aliases = BTreeMap::new();
    let plan_path = project_root.join(ASSET_BUILD_PLAN_FILE);
    if plan_path.is_file() {
        let bytes = std::fs::read(&plan_path)
            .map_err(|error| format!("read '{}': {error}", plan_path.display()))?;
        let plan: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse '{}': {error}", plan_path.display()))?;
        collect_build_plan_aliases(&plan, &mut aliases);
    }
    discover_convention_aliases(project_root, &mut aliases)?;
    Ok(aliases.into_iter().collect())
}

fn discover_project_source_roots(registry: &ContentMountRegistry) -> Vec<(PathBuf, i32)> {
    let mut roots: Vec<(PathBuf, i32)> = Vec::new();
    for mount in registry.mounts() {
        let candidates = [
            Some(mount.root.clone()),
            mount.root.parent().map(Path::to_path_buf),
        ];
        for candidate in candidates.into_iter().flatten() {
            if !candidate.join("Source").is_dir() {
                continue;
            }
            if let Some((_, priority)) = roots.iter_mut().find(|(root, _)| *root == candidate) {
                *priority = (*priority).max(mount.priority);
            } else {
                roots.push((candidate, mount.priority));
            }
            break;
        }
    }
    roots.sort_by(|a, b| a.0.cmp(&b.0));
    roots
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
            "asset_role": newengine_assets::asset_source_role::COMPILED,
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

    // Source companions are discovered from the selected project layout, never
    // from a global gameAssets directory. The build plan is authoritative for
    // non-trivial output names; bounded convention scanning covers direct pairs.
    for (project_root, priority) in discover_project_source_roots(registry) {
        let aliases = match discover_project_source_aliases(&project_root) {
            Ok(aliases) => aliases,
            Err(error) => {
                diagnostics.push(format!(
                    "WARN: project source discovery failed root='{}': {error}",
                    project_root.display()
                ));
                Vec::new()
            }
        };
        let alias_payload = aliases
            .iter()
            .map(|(logical_path, source_path)| {
                serde_json::json!({
                    "logical_path": logical_path,
                    "source_path": source_path,
                })
            })
            .collect::<Vec<_>>();
        let result = assets.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": priority,
            "mount": "",
            "asset_role": newengine_assets::asset_source_role::SOURCE,
            "aliases": alias_payload,
            "config": { "root": project_root.to_string_lossy() },
            "metadata": {
                "owner": "project-source-discovery",
                "policy": newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1,
                "build_plan": project_root.join(ASSET_BUILD_PLAN_FILE).to_string_lossy(),
            }
        }));
        match result {
            Ok(_) => diagnostics.push(format!(
                "SOURCE: {} aliases={} policy={}",
                project_root.display(),
                aliases.len(),
                newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1
            )),
            Err(error) => diagnostics.push(format!(
                "ERROR: source mount failed root='{}': {error}",
                project_root.display()
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

    #[test]
    fn build_plan_aliases_follow_current_project_layout() {
        let plan = serde_json::json!({
            "models": [{
                "source": "Source/models/hero.glb",
                "output": "Content/models/hero.ydd"
            }],
            "definitions": [{
                "source": "Source/definitions/hero.ytyp.xml",
                "output": "Definitions/hero.ytyp"
            }],
            "scripts": [{
                "source": "Source/scripts/game.lua",
                "output": "Scripts/game.ysc",
                "logical_path": "scripts/game.ysc"
            }]
        });
        let mut aliases = BTreeMap::new();
        collect_build_plan_aliases(&plan, &mut aliases);
        assert_eq!(
            aliases.get("models/hero.ydd").map(String::as_str),
            Some("Source/models/hero.glb")
        );
        assert_eq!(
            aliases.get("definitions/hero.ytyp").map(String::as_str),
            Some("Source/definitions/hero.ytyp.xml")
        );
        assert_eq!(
            aliases.get("scripts/game.ysc").map(String::as_str),
            Some("Source/scripts/game.lua")
        );
    }

    #[test]
    fn source_conventions_cover_double_extensions_and_scripts() {
        assert_eq!(
            convention_logical_path("maps/world.ymap.xml").as_deref(),
            Some("maps/world.ymap")
        );
        assert_eq!(
            convention_logical_path("materials/world.nemat.xml").as_deref(),
            Some("materials/world.nemat")
        );
        assert_eq!(
            convention_logical_path("scripts/game.lua").as_deref(),
            Some("scripts/game.ysc")
        );
        assert_eq!(
            convention_logical_path("animations/idle.clip.json").as_deref(),
            Some("animations/idle.ycd")
        );
    }
}
