use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const PROJECT_MANIFEST_FILE: &str = "game.toml";
pub const PROJECT_MANIFEST_CONTRACT: &str = "newengine.project.v1";
pub const PROJECT_STARTUP_SCENE_ENV: &str = "NEWENGINE_PROJECT_STARTUP_SCENE";
pub const PROJECT_LAUNCH_PRESET_ENV: &str = "NEWENGINE_PROJECT_LAUNCH_PRESET";
pub const PROJECT_RUNTIME_PROFILE_ABI_V1: &str = "newengine.runtime-profile/v1";

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentMountNamespace {
    Engine,
    Project,
    #[default]
    Game,
    Plugin,
    User,
}

impl ContentMountNamespace {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Project => "project",
            Self::Game => "game",
            Self::Plugin => "plugin",
            Self::User => "user",
        }
    }

    pub const fn default_priority(self) -> i32 {
        match self {
            Self::Engine => 100,
            Self::Plugin => 250,
            Self::Game => 300,
            Self::Project => 400,
            Self::User => 500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentMountDescriptor {
    pub id: String,
    pub namespace: ContentMountNamespace,
    pub root: PathBuf,
    pub mount: String,
    pub priority: i32,
    pub writable: bool,
    pub required: bool,
    pub owner: String,
}

impl Default for ContentMountDescriptor {
    fn default() -> Self {
        let namespace = ContentMountNamespace::Game;
        Self {
            id: String::new(),
            namespace,
            root: PathBuf::new(),
            mount: namespace.id().to_owned(),
            priority: namespace.default_priority(),
            writable: false,
            required: false,
            owner: String::new(),
        }
    }
}

impl ContentMountDescriptor {
    pub fn normalized(mut self, base: &Path) -> Self {
        if self.mount.trim().is_empty() {
            self.mount = self.namespace.id().to_owned();
        }
        if self.priority == 0 {
            self.priority = self.namespace.default_priority();
        }
        if self.root.is_relative() {
            self.root = base.join(&self.root);
        }
        self
    }

    pub fn logical_prefix(&self) -> String {
        format!("{}:/", self.namespace.id())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProjectContentMountState {
    pub required: bool,
    pub mounted: bool,
    pub attempts: u64,
    pub last_error: Option<String>,
}

impl ProjectContentMountState {
    pub fn pending() -> Self {
        Self {
            required: true,
            mounted: false,
            attempts: 0,
            last_error: None,
        }
    }

    pub const fn ready(&self) -> bool {
        !self.required || self.mounted
    }
}

#[derive(Clone, Debug, Default)]
pub struct ContentMountRegistry {
    mounts: Vec<ContentMountDescriptor>,
}

impl ContentMountRegistry {
    pub fn register(&mut self, descriptor: ContentMountDescriptor) -> Result<(), String> {
        let id = descriptor.id.trim();
        if id.is_empty() {
            return Err("content mount id must not be empty".to_owned());
        }
        if self.mounts.iter().any(|mount| mount.id == id) {
            return Err(format!("content mount already registered: {id}"));
        }
        self.mounts.push(descriptor);
        self.mounts
            .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }

    pub fn mounts(&self) -> &[ContentMountDescriptor] {
        &self.mounts
    }

    pub fn namespace_mounts(
        &self,
        namespace: ContentMountNamespace,
    ) -> impl Iterator<Item = &ContentMountDescriptor> {
        self.mounts
            .iter()
            .filter(move |mount| mount.namespace == namespace)
    }

    pub fn resolve_logical(&self, logical: &str) -> Option<PathBuf> {
        let (prefix, tail) = logical.split_once(":/")?;
        self.mounts.iter().find_map(|mount| {
            (mount.namespace.id() == prefix).then(|| {
                mount
                    .root
                    .join(tail.replace('/', std::path::MAIN_SEPARATOR_STR))
            })
        })
    }

    /// Resolve a physical file under the highest-priority registered mount back to a stable logical ref.
    /// Shadowed lower-priority files intentionally return `None`: changing them must not invalidate the
    /// currently winning VFS asset until mount priority changes.
    pub fn logical_for_physical(&self, physical: &Path) -> Option<String> {
        for mount in &self.mounts {
            let Ok(relative) = physical.strip_prefix(&mount.root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let tail = relative.to_string_lossy().replace('\\', "/");
            let logical = format!("{}:/{}", mount.namespace.id(), tail.trim_start_matches('/'));
            let Some(winner) = self.resolve_logical(&logical) else {
                continue;
            };
            if winner == physical {
                return Some(logical);
            }
        }
        None
    }

    /// Map a winning physical file to the actual AssetManager/VFS logical path.
    /// Unlike `logical_for_physical`, this uses the authored `mount` prefix (`game/foo`)
    /// instead of the editor-facing namespace syntax (`game:/foo`).
    pub fn asset_ref_for_physical(&self, physical: &Path) -> Option<String> {
        for mount in &self.mounts {
            let Ok(relative) = physical.strip_prefix(&mount.root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let tail = relative.to_string_lossy().replace('\\', "/");
            let mount_prefix = mount.mount.trim().trim_matches('/');
            let asset_ref = if mount_prefix.is_empty() {
                tail.trim_start_matches('/').to_owned()
            } else {
                format!("{mount_prefix}/{}", tail.trim_start_matches('/'))
            };
            if self.resolve_asset_ref(&asset_ref).as_deref() == Some(physical) {
                return Some(asset_ref);
            }
        }
        None
    }

    /// Resolve a provider/VFS asset ref through the same ordered mount semantics used
    /// when the project registry is mounted into `engine.assets`.
    pub fn resolve_asset_ref(&self, asset_ref: &str) -> Option<PathBuf> {
        let normalized = asset_ref.replace('\\', "/");
        for mount in &self.mounts {
            let prefix = mount.mount.trim().trim_matches('/');
            let relative = if prefix.is_empty() {
                normalized.as_str()
            } else {
                let wanted = format!("{prefix}/");
                let Some(relative) = normalized.strip_prefix(&wanted) else {
                    continue;
                };
                relative
            };
            if relative.is_empty() {
                continue;
            }
            return Some(
                mount
                    .root
                    .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)),
            );
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLaunchProfile {
    Editor,
    #[default]
    Game,
    Server,
    Test,
}

impl RuntimeLaunchProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Game => "game",
            Self::Server => "server",
            Self::Test => "test",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "editor" | "edit" => Some(Self::Editor),
            "game" | "play" => Some(Self::Game),
            "server" | "dedicated" | "headless" => Some(Self::Server),
            "test" | "smoke" => Some(Self::Test),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectLaunchPreset {
    /// Runtime mode selected by this preset. When omitted, the project-level
    /// `launch_profile` remains the default.
    pub profile: Option<RuntimeLaunchProfile>,
    /// Optional runtime composition/profile override. This is resolved through the
    /// runtime-profile registry instead of a launcher-side hardcoded branch.
    pub runtime_profile: Option<String>,
    /// Optional startup scene override for this launch preset.
    pub startup_scene: Option<String>,
    /// Optional authored UI presentation state override for this launch preset.
    pub startup_presentation_state: Option<String>,
}

impl ProjectLaunchPreset {
    pub fn validate(&self, id: &str) -> Result<(), String> {
        for (field, value) in [
            ("runtime_profile", self.runtime_profile.as_deref()),
            ("startup_scene", self.startup_scene.as_deref()),
            (
                "startup_presentation_state",
                self.startup_presentation_state.as_deref(),
            ),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(format!(
                    "launch preset '{id}' field '{field}' must be non-empty when specified"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjectLaunch {
    pub preset_id: String,
    pub profile: RuntimeLaunchProfile,
    pub runtime_profile: Option<String>,
    pub startup_scene: Option<String>,
    pub startup_presentation_state: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPluginRef {
    pub id: String,
    pub path: Option<PathBuf>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectScriptBinding {
    /// Module ID from `scripting.modules`, or a direct script asset reference.
    pub module: String,
    /// Optional exported operation used by the consumer. The project owns this name.
    pub operation: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectScriptingManifest {
    /// Optional scripting runtime/provider hint (for example `lua`).
    pub runtime: Option<String>,
    /// Optional bootstrap module ID or direct asset reference. No specific ID is required.
    pub entrypoint: Option<String>,
    /// Arbitrary module registry. Keys and count are entirely project-defined.
    pub modules: BTreeMap<String, String>,
    /// Arbitrary consumer -> module/operation bindings. Consumer IDs are provider/service contracts.
    pub bindings: BTreeMap<String, ProjectScriptBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjectScriptBinding {
    pub consumer: String,
    pub module_id: Option<String>,
    pub script_ref: String,
    pub operation: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectScriptRegistry {
    runtime: Option<String>,
    entrypoint: Option<String>,
    modules: BTreeMap<String, String>,
    bindings: BTreeMap<String, ProjectScriptBinding>,
}

impl ProjectScriptRegistry {
    pub fn from_manifest(manifest: &ProjectScriptingManifest) -> Result<Self, String> {
        validate_scripting_manifest(manifest)?;
        Ok(Self {
            runtime: manifest.runtime.clone(),
            entrypoint: manifest.entrypoint.clone(),
            modules: manifest.modules.clone(),
            bindings: manifest.bindings.clone(),
        })
    }

    #[inline]
    pub fn runtime(&self) -> Option<&str> {
        self.runtime.as_deref()
    }

    #[inline]
    pub fn module_ids(&self) -> impl Iterator<Item = &str> {
        self.modules.keys().map(String::as_str)
    }

    pub fn module_ref(&self, id: &str) -> Option<String> {
        self.modules
            .get(id.trim())
            .map(|value| normalize_script_ref(value))
    }

    pub fn resolve_ref_or_module(&self, value: &str) -> Option<(Option<String>, String)> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        if let Some(reference) = self.modules.get(value) {
            return Some((Some(value.to_owned()), normalize_script_ref(reference)));
        }
        Some((None, normalize_script_ref(value)))
    }

    pub fn entrypoint(&self) -> Option<String> {
        self.entrypoint
            .as_deref()
            .and_then(|value| self.resolve_ref_or_module(value))
            .map(|(_, reference)| reference)
    }

    pub fn binding(&self, consumer: &str) -> Option<ResolvedProjectScriptBinding> {
        let consumer = consumer.trim();
        let binding = self.bindings.get(consumer)?;
        let (module_id, script_ref) = self.resolve_ref_or_module(&binding.module)?;
        Some(ResolvedProjectScriptBinding {
            consumer: consumer.to_owned(),
            module_id,
            script_ref,
            operation: binding
                .operation
                .clone()
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

fn normalize_script_ref(value: &str) -> String {
    let value = value.trim().replace('\\', "/");
    if let Some((prefix, tail)) = value.split_once(":/") {
        format!(
            "{}/{}",
            prefix.trim_matches('/'),
            tail.trim_start_matches('/')
        )
    } else {
        value
    }
}

fn validate_scripting_manifest(manifest: &ProjectScriptingManifest) -> Result<(), String> {
    if manifest
        .runtime
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("scripting.runtime must be non-empty when specified".to_owned());
    }
    for (id, reference) in &manifest.modules {
        if id.trim().is_empty() {
            return Err("scripting.modules contains an empty module id".to_owned());
        }
        if reference.trim().is_empty() {
            return Err(format!(
                "scripting module '{id}' has an empty asset reference"
            ));
        }
    }
    if manifest
        .entrypoint
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("scripting.entrypoint must be non-empty when specified".to_owned());
    }
    for (consumer, binding) in &manifest.bindings {
        if consumer.trim().is_empty() {
            return Err("scripting.bindings contains an empty consumer id".to_owned());
        }
        if binding.module.trim().is_empty() {
            return Err(format!(
                "scripting binding '{consumer}' has an empty module"
            ));
        }
        if binding
            .operation
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(format!(
                "scripting binding '{consumer}' has an empty operation"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectManifest {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    /// Launcher executable/package used by the engine Project Browser. This is
    /// intentionally separate from `game_module`: the launcher owns process/profile
    /// composition while the game module remains a runtime plugin contract.
    pub launcher: Option<String>,
    /// Built-in runtime profile id handled by the generic NewEngine executable.
    /// Arbitrary ids are allowed; the launcher resolves only profiles compiled/registered in the host.
    pub runtime_profile: Option<String>,
    pub game_module: Option<String>,
    pub startup_scene: Option<String>,
    pub launch_profile: Option<RuntimeLaunchProfile>,
    /// Optional authored UI presentation state selected when this project boots.
    /// Examples: `main_menu`, `gameplay`, `loading`. The project stays data-driven;
    /// runtime-host only projects this value into the generic screen-profile config.
    pub startup_presentation_state: Option<String>,
    /// Optional named launch preset selected when no explicit `--launch` request is supplied.
    pub default_launch: Option<String>,
    /// Named project launch presets. Typical ids are `editor`, `game`, `server`, and `test`,
    /// but the ids are project-owned. Each preset overlays the project-level defaults.
    pub launch: BTreeMap<String, ProjectLaunchPreset>,
    pub scripting: ProjectScriptingManifest,
    pub content: Vec<ContentMountDescriptor>,
    pub definitions: Vec<PathBuf>,
    pub scripts: Vec<PathBuf>,
    pub plugins: Vec<ProjectPluginRef>,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            format_version: 1,
            id: String::new(),
            name: String::new(),
            launcher: None,
            runtime_profile: None,
            game_module: None,
            startup_scene: None,
            launch_profile: None,
            startup_presentation_state: None,
            default_launch: None,
            launch: BTreeMap::new(),
            scripting: ProjectScriptingManifest::default(),
            content: vec![ContentMountDescriptor {
                id: "game.content".to_owned(),
                namespace: ContentMountNamespace::Game,
                root: PathBuf::from("content"),
                mount: "game".to_owned(),
                priority: ContentMountNamespace::Game.default_priority(),
                writable: false,
                required: false,
                owner: "project".to_owned(),
            }],
            definitions: vec![PathBuf::from("definitions")],
            scripts: vec![PathBuf::from("scripts")],
            plugins: Vec::new(),
        }
    }
}

impl ProjectManifest {
    pub fn resolve_launch(&self, requested: Option<&str>) -> Result<ResolvedProjectLaunch, String> {
        let requested = requested.map(str::trim).filter(|value| !value.is_empty());
        let selected_id = requested.map(str::to_owned).or_else(|| {
            self.default_launch
                .clone()
                .filter(|value| !value.trim().is_empty())
        });

        let (preset_id, preset, synthetic_profile) = if let Some(id) = selected_id.as_deref() {
            if let Some(preset) = self.launch.get(id) {
                (id.to_owned(), Some(preset), None)
            } else if let Some(profile) = RuntimeLaunchProfile::parse(id) {
                // Standard launch modes are always legal overrides even for legacy v1 manifests.
                (id.to_owned(), None, Some(profile))
            } else {
                return Err(format!(
                    "project launch preset '{id}' is not declared and is not a standard launch mode"
                ));
            }
        } else {
            (
                self.launch_profile.unwrap_or_default().id().to_owned(),
                None,
                None,
            )
        };

        let profile = preset
            .and_then(|preset| preset.profile)
            .or(synthetic_profile)
            .or(self.launch_profile)
            .unwrap_or_default();
        let runtime_profile = preset
            .and_then(|preset| preset.runtime_profile.clone())
            .or_else(|| self.runtime_profile.clone());
        let startup_scene = preset
            .and_then(|preset| preset.startup_scene.clone())
            .or_else(|| self.startup_scene.clone());
        let startup_presentation_state = preset
            .and_then(|preset| preset.startup_presentation_state.clone())
            .or_else(|| self.startup_presentation_state.clone());

        Ok(ResolvedProjectLaunch {
            preset_id,
            profile,
            runtime_profile,
            startup_scene,
            startup_presentation_state,
        })
    }

    pub fn launch_ids(&self) -> Vec<String> {
        if self.launch.is_empty() {
            return vec![self.launch_profile.unwrap_or_default().id().to_owned()];
        }
        self.launch.keys().cloned().collect()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.format_version != 1 {
            errors.push(format!(
                "unsupported project format_version={}",
                self.format_version
            ));
        }
        if self.id.trim().is_empty() {
            errors.push("project id must not be empty".to_owned());
        }
        if self
            .launcher
            .as_ref()
            .is_some_and(|launcher| launcher.trim().is_empty())
        {
            errors.push("project launcher must be non-empty when specified".to_owned());
        }
        if self
            .runtime_profile
            .as_ref()
            .is_some_and(|profile| profile.trim().is_empty())
        {
            errors.push("project runtime_profile must be non-empty when specified".to_owned());
        }
        if self
            .startup_presentation_state
            .as_ref()
            .is_some_and(|state| state.trim().is_empty())
        {
            errors.push(
                "project startup_presentation_state must be non-empty when specified".to_owned(),
            );
        }
        if self
            .default_launch
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            errors.push("project default_launch must be non-empty when specified".to_owned());
        }
        if let Some(default_launch) = self.default_launch.as_deref() {
            let default_launch = default_launch.trim();
            if !default_launch.is_empty()
                && !self.launch.contains_key(default_launch)
                && RuntimeLaunchProfile::parse(default_launch).is_none()
            {
                errors.push(format!(
                    "project default_launch '{default_launch}' is neither a declared preset nor a standard launch mode"
                ));
            }
        }
        for (id, preset) in &self.launch {
            if id.trim().is_empty() {
                errors.push("project launch contains an empty preset id".to_owned());
                continue;
            }
            if let Err(error) = preset.validate(id) {
                errors.push(error);
            }
        }
        if let Err(error) = validate_scripting_manifest(&self.scripting) {
            errors.push(error);
        }
        let mut ids = std::collections::BTreeSet::new();
        for mount in &self.content {
            if mount.id.trim().is_empty() {
                errors.push("content mount id must not be empty".to_owned());
            } else if !ids.insert(mount.id.trim().to_owned()) {
                errors.push(format!("duplicate content mount id '{}'", mount.id));
            }
            if mount.root.as_os_str().is_empty() {
                errors.push(format!(
                    "content mount '{}' root must not be empty",
                    mount.id
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_manifest_accepts_direct_gameplay_presentation_state() {
        let manifest = ProjectManifest {
            id: "direct-game".to_owned(),
            name: "Direct Game".to_owned(),
            launch_profile: Some(RuntimeLaunchProfile::Game),
            startup_presentation_state: Some("gameplay".to_owned()),
            ..ProjectManifest::default()
        };
        assert_eq!(manifest.launch_profile, Some(RuntimeLaunchProfile::Game));
        assert_eq!(
            manifest.startup_presentation_state.as_deref(),
            Some("gameplay")
        );
        manifest
            .validate()
            .expect("valid direct-game project manifest");
    }

    #[test]
    fn script_registry_accepts_arbitrary_module_ids_and_bindings() {
        let manifest = ProjectScriptingManifest {
            runtime: Some("lua".to_owned()),
            entrypoint: Some("boot".to_owned()),
            modules: BTreeMap::from([
                ("boot".to_owned(), "scripts:/game.ysc".to_owned()),
                (
                    "my_weird_data".to_owned(),
                    "scripts:/custom/data.ysc".to_owned(),
                ),
            ]),
            bindings: BTreeMap::from([(
                "consumer.anything".to_owned(),
                ProjectScriptBinding {
                    module: "my_weird_data".to_owned(),
                    operation: Some("produce_whatever".to_owned()),
                },
            )]),
        };
        let registry = ProjectScriptRegistry::from_manifest(&manifest).unwrap();
        assert_eq!(registry.entrypoint().as_deref(), Some("scripts/game.ysc"));
        let binding = registry.binding("consumer.anything").unwrap();
        assert_eq!(binding.script_ref, "scripts/custom/data.ysc");
        assert_eq!(binding.operation.as_deref(), Some("produce_whatever"));
    }

    #[test]
    fn launch_presets_overlay_project_defaults() {
        let manifest = ProjectManifest {
            id: "launchable".to_owned(),
            runtime_profile: Some("runtime.default".to_owned()),
            startup_scene: Some("maps/default.ymap".to_owned()),
            launch_profile: Some(RuntimeLaunchProfile::Game),
            launch: BTreeMap::from([(
                "editor".to_owned(),
                ProjectLaunchPreset {
                    profile: Some(RuntimeLaunchProfile::Editor),
                    runtime_profile: None,
                    startup_scene: Some("maps/edit.ymap".to_owned()),
                    startup_presentation_state: None,
                },
            )]),
            ..ProjectManifest::default()
        };
        let resolved = manifest.resolve_launch(Some("editor")).unwrap();
        assert_eq!(resolved.profile, RuntimeLaunchProfile::Editor);
        assert_eq!(resolved.runtime_profile.as_deref(), Some("runtime.default"));
        assert_eq!(resolved.startup_scene.as_deref(), Some("maps/edit.ymap"));
    }

    #[test]
    fn standard_launch_modes_override_legacy_manifest_without_authored_presets() {
        let manifest = ProjectManifest {
            id: "legacy".to_owned(),
            launch_profile: Some(RuntimeLaunchProfile::Game),
            ..ProjectManifest::default()
        };
        let resolved = manifest.resolve_launch(Some("server")).unwrap();
        assert_eq!(resolved.profile, RuntimeLaunchProfile::Server);
        assert_eq!(resolved.preset_id, "server");
    }

    #[test]
    fn registry_resolves_namespace_paths_by_priority_order() {
        let mut registry = ContentMountRegistry::default();
        registry
            .register(ContentMountDescriptor {
                id: "game".to_owned(),
                root: PathBuf::from("C:/project/content"),
                ..ContentMountDescriptor::default()
            })
            .unwrap();
        assert_eq!(
            registry.resolve_logical("game:/models/a.nef8"),
            Some(
                PathBuf::from("C:/project/content")
                    .join("models")
                    .join("a.nef8")
            )
        );
        assert_eq!(
            registry.logical_for_physical(
                &PathBuf::from("C:/project/content")
                    .join("models")
                    .join("a.nef8")
            ),
            Some("game:/models/a.nef8".to_owned())
        );
        assert_eq!(
            registry.asset_ref_for_physical(
                &PathBuf::from("C:/project/content")
                    .join("models")
                    .join("a.nef8")
            ),
            Some("game/models/a.nef8".to_owned())
        );
        assert_eq!(
            registry.resolve_asset_ref("game/models/a.nef8"),
            Some(
                PathBuf::from("C:/project/content")
                    .join("models")
                    .join("a.nef8")
            )
        );
    }
}
