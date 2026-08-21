use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::scripting::validate_scripting_manifest;
use crate::validation::collect_optional_non_blank;
use crate::{
    ContentMountDescriptor, ContentMountNamespace, ProjectLaunchPreset, ProjectPluginRef,
    ProjectScriptingManifest, ResolvedProjectLaunch, RuntimeLaunchProfile,
};

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
        collect_optional_non_blank(&mut errors, self.launcher.as_deref(), || {
            "project launcher must be non-empty when specified".to_owned()
        });
        collect_optional_non_blank(&mut errors, self.runtime_profile.as_deref(), || {
            "project runtime_profile must be non-empty when specified".to_owned()
        });
        collect_optional_non_blank(
            &mut errors,
            self.startup_presentation_state.as_deref(),
            || "project startup_presentation_state must be non-empty when specified".to_owned(),
        );
        collect_optional_non_blank(&mut errors, self.default_launch.as_deref(), || {
            "project default_launch must be non-empty when specified".to_owned()
        });
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
        let mut ids = BTreeSet::new();
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
