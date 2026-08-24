use std::collections::BTreeMap;

use newengine_ui_api::UiInputFrame;
use serde::{Deserialize, Serialize};

pub mod editor_command {
    pub const RUNTIME_STOP: &str = "editor.runtime.stop";
    pub const RUNTIME_SIMULATE: &str = "editor.runtime.simulate";
    pub const RUNTIME_PLAY: &str = "editor.runtime.play";
    pub const RUNTIME_PAUSE: &str = "editor.runtime.pause";
    pub const RUNTIME_RESTART: &str = "editor.runtime.restart";
    pub const RUNTIME_STEP: &str = "editor.runtime.step";
    pub const RUNTIME_EJECT: &str = "editor.runtime.eject";
    pub const RUNTIME_POSSESS: &str = "editor.runtime.possess";
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorShortcut {
    pub key: u32,
    pub required_down: Vec<u32>,
    pub display: String,
}

impl EditorShortcut {
    pub fn key(key: u32, display: impl Into<String>) -> Self {
        Self {
            key,
            required_down: Vec::new(),
            display: display.into(),
        }
    }

    pub fn matches(&self, input: &UiInputFrame) -> bool {
        input.is_key_pressed(self.key)
            && self
                .required_down
                .iter()
                .all(|modifier| input.is_key_down(*modifier))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorCommandEnablePolicy {
    #[default]
    Always,
    RuntimeActive,
    RuntimePaused,
    RuntimeInactive,
    RuntimePlay,
    RuntimePossessed,
    RuntimeEjected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorCommandContext {
    pub runtime_active: bool,
    pub runtime_paused: bool,
    pub runtime_playing: bool,
    pub runtime_possessed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorCommandDescriptor {
    pub id: String,
    pub label: String,
    pub category: String,
    pub tooltip: String,
    pub priority: i32,
    pub shortcut: Option<EditorShortcut>,
    pub enable_policy: EditorCommandEnablePolicy,
}

impl Default for EditorCommandDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            category: "General".to_owned(),
            tooltip: String::new(),
            priority: 0,
            shortcut: None,
            enable_policy: EditorCommandEnablePolicy::Always,
        }
    }
}

impl EditorCommandDescriptor {
    pub fn enabled(&self, context: EditorCommandContext) -> bool {
        match self.enable_policy {
            EditorCommandEnablePolicy::Always => true,
            EditorCommandEnablePolicy::RuntimeActive => context.runtime_active,
            EditorCommandEnablePolicy::RuntimePaused => {
                context.runtime_active && context.runtime_paused
            }
            EditorCommandEnablePolicy::RuntimeInactive => !context.runtime_active,
            EditorCommandEnablePolicy::RuntimePlay => {
                context.runtime_active && context.runtime_playing
            }
            EditorCommandEnablePolicy::RuntimePossessed => {
                context.runtime_active && context.runtime_playing && context.runtime_possessed
            }
            EditorCommandEnablePolicy::RuntimeEjected => {
                context.runtime_active && context.runtime_playing && !context.runtime_possessed
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EditorCommandRegistry {
    commands: BTreeMap<String, EditorCommandDescriptor>,
}

impl EditorCommandRegistry {
    pub fn register(&mut self, descriptor: EditorCommandDescriptor) -> Result<(), String> {
        let id = descriptor.id.trim();
        if id.is_empty() {
            return Err("editor command id must not be empty".to_owned());
        }
        if self.commands.contains_key(id) {
            return Err(format!("editor command already registered: {id}"));
        }
        self.commands.insert(id.to_owned(), descriptor);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&EditorCommandDescriptor> {
        self.commands.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &EditorCommandDescriptor> {
        self.commands.values()
    }

    pub fn resolve_pressed(
        &self,
        input: &UiInputFrame,
        context: EditorCommandContext,
    ) -> Option<&EditorCommandDescriptor> {
        self.commands
            .values()
            .filter(|command| command.enabled(context))
            .filter(|command| {
                command
                    .shortcut
                    .as_ref()
                    .is_some_and(|it| it.matches(input))
            })
            .max_by_key(|command| command.priority)
    }
}

pub fn default_runtime_editor_commands() -> EditorCommandRegistry {
    use editor_command::*;
    use newengine_input_api::key_code::{DIGIT1, DIGIT2, DIGIT3, DIGIT4, DIGIT5, SPACE};

    let mut registry = EditorCommandRegistry::default();
    let commands = [
        EditorCommandDescriptor {
            id: RUNTIME_STOP.to_owned(),
            label: "Stop".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Stop the active session and restore the editor world".to_owned(),
            priority: 100,
            shortcut: Some(EditorShortcut::key(DIGIT1, "1")),
            enable_policy: EditorCommandEnablePolicy::RuntimeActive,
        },
        EditorCommandDescriptor {
            id: RUNTIME_SIMULATE.to_owned(),
            label: "Simulate".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Run simulation without direct player possession".to_owned(),
            priority: 100,
            shortcut: Some(EditorShortcut::key(DIGIT2, "2")),
            enable_policy: EditorCommandEnablePolicy::Always,
        },
        EditorCommandDescriptor {
            id: RUNTIME_PLAY.to_owned(),
            label: "Play".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Start Play In Editor".to_owned(),
            priority: 100,
            shortcut: Some(EditorShortcut::key(DIGIT3, "3")),
            enable_policy: EditorCommandEnablePolicy::Always,
        },
        EditorCommandDescriptor {
            id: RUNTIME_PAUSE.to_owned(),
            label: "Pause".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Pause or resume the active runtime session".to_owned(),
            priority: 110,
            shortcut: Some(EditorShortcut::key(SPACE, "Space")),
            enable_policy: EditorCommandEnablePolicy::RuntimeActive,
        },
        EditorCommandDescriptor {
            id: RUNTIME_RESTART.to_owned(),
            label: "Restart".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Restore the editor snapshot and start a fresh session".to_owned(),
            priority: 90,
            shortcut: Some(EditorShortcut::key(DIGIT4, "4")),
            enable_policy: EditorCommandEnablePolicy::RuntimeActive,
        },
        EditorCommandDescriptor {
            id: RUNTIME_STEP.to_owned(),
            label: "Step".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Advance exactly one fixed simulation step while paused".to_owned(),
            priority: 120,
            shortcut: Some(EditorShortcut::key(DIGIT5, "5")),
            enable_policy: EditorCommandEnablePolicy::RuntimePaused,
        },
        EditorCommandDescriptor {
            id: RUNTIME_EJECT.to_owned(),
            label: "Eject".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Release player possession while the live runtime continues".to_owned(),
            priority: 95,
            shortcut: None,
            enable_policy: EditorCommandEnablePolicy::RuntimePossessed,
        },
        EditorCommandDescriptor {
            id: RUNTIME_POSSESS.to_owned(),
            label: "Possess".to_owned(),
            category: "Runtime".to_owned(),
            tooltip: "Return control to the player in the active live runtime".to_owned(),
            priority: 95,
            shortcut: None,
            enable_policy: EditorCommandEnablePolicy::RuntimeEjected,
        },
    ];
    for command in commands {
        registry
            .register(command)
            .expect("built-in editor command ids are unique");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_step_resolves_only_when_runtime_is_paused() {
        let registry = default_runtime_editor_commands();
        let mut input = UiInputFrame::default();
        input
            .keys_pressed
            .insert(newengine_input_api::key_code::DIGIT5);
        assert!(registry
            .resolve_pressed(&input, EditorCommandContext::default())
            .is_none());
        let command = registry
            .resolve_pressed(
                &input,
                EditorCommandContext {
                    runtime_active: true,
                    runtime_paused: true,
                    ..EditorCommandContext::default()
                },
            )
            .expect("step command");
        assert_eq!(command.id, editor_command::RUNTIME_STEP);
    }
}
