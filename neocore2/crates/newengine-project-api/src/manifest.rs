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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectUiInputFocusPolicy {
    EditorShell,
    #[default]
    UiSurface,
    GameViewport,
    Headless,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectUiPresentationStateManifest {
    pub id: String,
    pub document_ref: Option<String>,
    pub surface_id: Option<String>,
    pub input_focus_policy: ProjectUiInputFocusPolicy,
    pub blocks_world_bootstrap: bool,
    pub blocks_gameplay_input: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectUiPresentationTransitionManifest {
    pub from: String,
    pub to: String,
    pub on_action: Option<String>,
    pub on_runtime_ready: bool,
    pub reset_runtime_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectUiPresentationFlowManifest {
    pub enabled: bool,
    pub id: String,
    pub initial_state: String,
    pub states: Vec<ProjectUiPresentationStateManifest>,
    pub transitions: Vec<ProjectUiPresentationTransitionManifest>,
}

impl Default for ProjectUiPresentationFlowManifest {
    fn default() -> Self {
        Self {
            enabled: true,
            id: String::new(),
            initial_state: String::new(),
            states: Vec::new(),
            transitions: Vec::new(),
        }
    }
}

impl ProjectUiPresentationFlowManifest {
    fn has_state(&self, state_id: &str) -> bool {
        let state_id = state_id.trim();
        self.states.iter().any(|state| state.id.trim() == state_id)
    }

    fn validate(&self, errors: &mut Vec<String>) {
        if !self.enabled {
            return;
        }
        if self.id.trim().is_empty() {
            errors.push("project ui.presentation_flow.id must not be empty".to_owned());
        }
        let initial = self.initial_state.trim();
        if initial.is_empty() {
            errors.push("project ui.presentation_flow.initial_state must not be empty".to_owned());
        }

        let mut state_ids = BTreeSet::new();
        for (index, state) in self.states.iter().enumerate() {
            let id = state.id.trim();
            if id.is_empty() {
                errors.push(format!(
                    "project ui.presentation_flow.states[{index}].id must not be empty"
                ));
                continue;
            }
            if !state_ids.insert(id.to_owned()) {
                errors.push(format!(
                    "project ui.presentation_flow contains duplicate state '{id}'"
                ));
            }
            let document = state
                .document_ref
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let surface = state
                .surface_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            if state.document_ref.is_some() && document.is_none() {
                errors.push(format!(
                    "project ui presentation state '{id}' has blank document_ref"
                ));
            }
            if state.surface_id.is_some() && surface.is_none() {
                errors.push(format!(
                    "project ui presentation state '{id}' has blank surface_id"
                ));
            }
            if document.is_some() != surface.is_some() {
                errors.push(format!("project ui presentation state '{id}' must declare document_ref and surface_id together"));
            }
            if let Some(document) = document {
                let normalized = document.replace('\\', "/").to_ascii_lowercase();
                if normalized.starts_with("ui/engine/")
                    || normalized.starts_with("assets/ui/engine/")
                {
                    errors.push(format!("project ui presentation state '{id}' references engine-owned UI '{document}'; game frontend documents must be project-owned"));
                }
            }
            if state.input_focus_policy == ProjectUiInputFocusPolicy::GameViewport
                && state.blocks_gameplay_input
            {
                errors.push(format!("project ui presentation state '{id}' uses game_viewport focus while blocking gameplay input"));
            }
        }
        if !initial.is_empty() && !state_ids.contains(initial) {
            errors.push(format!(
                "project ui.presentation_flow.initial_state '{initial}' is not declared"
            ));
        }

        let mut action_triggers = BTreeSet::new();
        let mut ready_triggers = BTreeSet::new();
        for (index, transition) in self.transitions.iter().enumerate() {
            let from = transition.from.trim();
            let to = transition.to.trim();
            if from.is_empty() || !state_ids.contains(from) {
                errors.push(format!(
                    "project ui presentation transition[{index}] has unknown from state '{from}'"
                ));
            }
            if to.is_empty() || !state_ids.contains(to) {
                errors.push(format!(
                    "project ui presentation transition[{index}] has unknown to state '{to}'"
                ));
            }
            let action = transition
                .on_action
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            if transition.on_action.is_some() && action.is_none() {
                errors.push(format!(
                    "project ui presentation transition[{index}] has blank on_action"
                ));
            }
            if action.is_some() == transition.on_runtime_ready {
                errors.push(format!("project ui presentation transition[{index}] must declare exactly one trigger: on_action or on_runtime_ready"));
            }
            if let Some(action) = action {
                if !action_triggers.insert((from.to_owned(), action.to_owned())) {
                    errors.push(format!("project ui presentation flow has ambiguous action '{action}' from state '{from}'"));
                }
            }
            if transition.on_runtime_ready && !ready_triggers.insert(from.to_owned()) {
                errors.push(format!("project ui presentation flow has multiple runtime-ready transitions from '{from}'"));
            }
        }

        if errors.is_empty() && !state_ids.is_empty() {
            let mut reachable = BTreeSet::new();
            let mut pending = vec![initial.to_owned()];
            while let Some(current) = pending.pop() {
                if !reachable.insert(current.clone()) {
                    continue;
                }
                for transition in &self.transitions {
                    if transition.from.trim() == current {
                        pending.push(transition.to.trim().to_owned());
                    }
                }
            }
            for state in state_ids.difference(&reachable) {
                errors.push(format!(
                    "project ui presentation state '{state}' is unreachable from '{initial}'"
                ));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectUiSharedManifest {
    /// Enables the suite-owned Shared UI composition for this project.
    pub enabled: bool,
    /// Adds the canonical Shared ESC/pause menu to playable game states.
    pub pause_menu: bool,
    /// Supplies the canonical Shared HUD only when the project does not author one.
    pub hud_fallback: bool,
    /// Optional Shared pause document override. Omitted values use the runtime convention.
    pub pause_document_ref: Option<String>,
    /// Optional Shared HUD document override. Omitted values use the runtime convention.
    pub hud_document_ref: Option<String>,
}

impl Default for ProjectUiSharedManifest {
    fn default() -> Self {
        Self {
            enabled: true,
            pause_menu: true,
            hud_fallback: true,
            pause_document_ref: None,
            hud_document_ref: None,
        }
    }
}

impl ProjectUiSharedManifest {
    fn validate(&self, errors: &mut Vec<String>) {
        for (field, value) in [
            ("pause_document_ref", self.pause_document_ref.as_deref()),
            ("hud_document_ref", self.hud_document_ref.as_deref()),
        ] {
            collect_optional_non_blank(errors, value, || {
                format!("project ui.shared.{field} must be non-empty when specified")
            });
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectUiManifest {
    /// Generic engine screen-profile id (for example `game` or `headless`).
    pub screen_profile: Option<String>,
    /// Project-owned root UI surface id.
    pub root_surface: Option<String>,
    /// Project asset reference for the root UI document.
    pub document: Option<String>,
    /// Project-owned frontend/presentation state graph. The engine only executes this graph.
    pub presentation_flow: Option<ProjectUiPresentationFlowManifest>,
    /// Suite-owned Shared UI inherited by projects unless explicitly disabled.
    pub shared: ProjectUiSharedManifest,
    /// Whether runtime should publish the editor shell alongside project UI.
    pub publish_editor_shell: Option<bool>,
}

impl ProjectUiManifest {
    fn validate(&self, errors: &mut Vec<String>) {
        for (field, value) in [
            ("screen_profile", self.screen_profile.as_deref()),
            ("root_surface", self.root_surface.as_deref()),
            ("document", self.document.as_deref()),
        ] {
            collect_optional_non_blank(errors, value, || {
                format!("project ui.{field} must be non-empty when specified")
            });
        }
        if let Some(flow) = self.presentation_flow.as_ref() {
            flow.validate(errors);
        }
        self.shared.validate(errors);
    }
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
    /// Project-owned runtime UI composition. Runtime profiles must not hardcode game HUD assets.
    pub ui: ProjectUiManifest,
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
            ui: ProjectUiManifest::default(),
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
            definitions: vec![PathBuf::from("Content/definitions")],
            scripts: vec![PathBuf::from("Content/scripts")],
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
        self.ui.validate(&mut errors);
        if let Some(flow) = self
            .ui
            .presentation_flow
            .as_ref()
            .filter(|flow| flow.enabled)
        {
            if let Some(state) = self.startup_presentation_state.as_deref() {
                if !flow.has_state(state) {
                    errors.push(format!(
                        "project startup_presentation_state '{}' is not declared by ui.presentation_flow",
                        state.trim()
                    ));
                }
            }
            for (launch_id, preset) in &self.launch {
                if let Some(state) = preset.startup_presentation_state.as_deref() {
                    if !flow.has_state(state) {
                        errors.push(format!(
                            "project launch '{launch_id}' startup_presentation_state '{}' is not declared by ui.presentation_flow",
                            state.trim()
                        ));
                    }
                }
            }
        }
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
