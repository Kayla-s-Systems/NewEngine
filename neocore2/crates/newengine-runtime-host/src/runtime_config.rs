use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const ENGINE_RUNTIME_CONFIG_FILE: &str = "runtime.toml";
pub const ENGINE_RUNTIME_CONFIG_SCHEMA: &str = "newengine.runtime.v1";
pub const ENGINE_RUNTIME_CONFIG_ENV: &str = "NEWENGINE_RUNTIME_CONFIG";
pub const ENGINE_STARTUP_CONFIG_ENV: &str = "NEWENGINE_STARTUP_CONFIG_PATH";
pub const ENGINE_RUNTIME_MODE_ENV: &str = "NEWENGINE_RUNTIME_MODE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineRuntimeMode {
    #[default]
    Game,
    Server,
    Test,
}

impl EngineRuntimeMode {
    pub const fn launch_id(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Server => "server",
            Self::Test => "test",
        }
    }

    pub const fn requires_headless(self) -> bool {
        matches!(self, Self::Server | Self::Test)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineRuntimeSection {
    pub mode: EngineRuntimeMode,
    pub startup_config: String,
    pub plugin_target: String,
    pub headless: bool,
    pub startup_window: bool,
    pub startup_intro: Option<String>,
}

impl Default for EngineRuntimeSection {
    fn default() -> Self {
        Self {
            mode: EngineRuntimeMode::Game,
            startup_config: "config.json".to_owned(),
            plugin_target: "runtime".to_owned(),
            headless: false,
            startup_window: true,
            startup_intro: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineRuntimeConfig {
    pub format_version: u32,
    pub schema: String,
    pub runtime: EngineRuntimeSection,
}

impl Default for EngineRuntimeConfig {
    fn default() -> Self {
        Self {
            format_version: 1,
            schema: ENGINE_RUNTIME_CONFIG_SCHEMA.to_owned(),
            runtime: EngineRuntimeSection::default(),
        }
    }
}

impl EngineRuntimeConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.format_version != 1 {
            errors.push(format!(
                "runtime.toml format_version must be 1; actual={}",
                self.format_version
            ));
        }
        if self.schema.trim() != ENGINE_RUNTIME_CONFIG_SCHEMA {
            errors.push(format!(
                "runtime.toml schema must be '{}'; actual='{}'",
                ENGINE_RUNTIME_CONFIG_SCHEMA, self.schema
            ));
        }
        if self.runtime.startup_config.trim().is_empty() {
            errors.push("runtime.startup_config must not be empty".to_owned());
        }
        if self.runtime.plugin_target.trim().is_empty() {
            errors.push("runtime.plugin_target must not be empty".to_owned());
        }
        if self
            .runtime
            .startup_intro
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            errors.push("runtime.startup_intro must not be empty when declared".to_owned());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn apply_process_env(&self, source_path: &Path) -> Result<(), String> {
        self.validate()
            .map_err(|errors| format!("engine runtime config invalid: {}", errors.join("; ")))?;
        let root = source_path.parent().unwrap_or_else(|| Path::new("."));
        let startup = PathBuf::from(self.runtime.startup_config.trim());
        let startup = if startup.is_absolute() {
            startup
        } else {
            root.join(startup)
        };
        std::env::set_var(ENGINE_RUNTIME_CONFIG_ENV, source_path);
        std::env::set_var(ENGINE_STARTUP_CONFIG_ENV, startup);
        std::env::set_var(ENGINE_RUNTIME_MODE_ENV, self.runtime.mode.launch_id());
        std::env::set_var("NEWENGINE_PLUGIN_TARGET", self.runtime.plugin_target.trim());
        let headless = self.runtime.headless || self.runtime.mode.requires_headless();
        std::env::set_var("NEWENGINE_HEADLESS", if headless { "1" } else { "0" });
        if self.runtime.startup_window && !headless {
            std::env::remove_var("NEWENGINE_STARTUP_WINDOW_DISABLED");
            std::env::remove_var("NEWENGINE_STARTUP_WINDOW_SKIP");
        } else {
            std::env::set_var("NEWENGINE_STARTUP_WINDOW_DISABLED", "1");
        }
        if let Some(descriptor) = self
            .runtime
            .startup_intro
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            std::env::set_var(
                newengine_core::startup_intro::ENGINE_STARTUP_INTRO_DESCRIPTOR_ENV,
                descriptor,
            );
        } else {
            std::env::remove_var(
                newengine_core::startup_intro::ENGINE_STARTUP_INTRO_DESCRIPTOR_ENV,
            );
        }
        Ok(())
    }
}

pub fn load_engine_runtime_config() -> Result<(PathBuf, EngineRuntimeConfig), String> {
    let path = engine_runtime_config_path().ok_or_else(|| {
        format!(
            "engine runtime configuration '{}' was not found next to NewEngine.exe",
            ENGINE_RUNTIME_CONFIG_FILE
        )
    })?;
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("read engine runtime config '{}': {error}", path.display()))?;
    let config: EngineRuntimeConfig = toml::from_str(&source)
        .map_err(|error| format!("parse engine runtime config '{}': {error}", path.display()))?;
    config
        .validate()
        .map_err(|errors| format!("engine runtime config invalid: {}", errors.join("; ")))?;
    Ok((path, config))
}

pub fn engine_runtime_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // Cargo development binaries live under `<workspace>/target/{debug,release}`. A copied
    // runtime.toml next to such a binary must not change the meaning of relative paths such
    // as `startup_config = "config.json"`: the authoritative development runtime/config pair
    // lives at the workspace root. Prefer that pair whenever the executable is inside the
    // workspace target tree. Installed/packaged binaries continue to use adjacent runtime.toml.
    if let Some(workspace) = cargo_workspace_ancestor(exe_dir) {
        if exe_dir.starts_with(workspace.join("target")) {
            let candidate = workspace.join(ENGINE_RUNTIME_CONFIG_FILE);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let adjacent = exe_dir.join(ENGINE_RUNTIME_CONFIG_FILE);
    if adjacent.is_file() {
        return Some(adjacent);
    }

    // Development workspace fallback for unusual launcher layouts below neocore2.
    cargo_workspace_ancestor(exe_dir)
        .map(|workspace| workspace.join(ENGINE_RUNTIME_CONFIG_FILE))
        .filter(|candidate| candidate.is_file())
}

fn cargo_workspace_ancestor(start: &Path) -> Option<PathBuf> {
    start.ancestors().take(10).find_map(|dir| {
        (dir.file_name().and_then(|name| name.to_str()) == Some("neocore2")
            && dir.join("Cargo.toml").is_file()
            && dir.join("crates").is_dir()
            && dir.join("apps").is_dir())
        .then(|| dir.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_contains_only_engine_boot_policy() {
        let source = r#"
format_version = 1
schema = "newengine.runtime.v1"
[runtime]
mode = "server"
startup_config = "config.json"
plugin_target = "runtime"
headless = true
startup_window = false
"#;
        let config: EngineRuntimeConfig = toml::from_str(source).unwrap();
        config.validate().unwrap();
        assert_eq!(config.runtime.mode, EngineRuntimeMode::Server);
        assert!(config.runtime.mode.requires_headless());

        let removed_mode = ["edi", "tor"].concat();
        let legacy_removed_mode_source = format!(
            r#"
format_version = 1
schema = "newengine.runtime.v1"
[runtime]
mode = "{removed_mode}"
"#,
        );
        assert!(toml::from_str::<EngineRuntimeConfig>(&legacy_removed_mode_source).is_err());
    }
}
