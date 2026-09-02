#![forbid(unsafe_op_in_unsafe_fn)]

use std::{env, path::PathBuf};

pub use newengine_startup_intro::{
    StartupIntroNativeBackend, StartupIntroNativeWindow, StartupIntroReport, StartupIntroStatus,
};

/// Runtime boot policy ingress. `runtime.toml` publishes only the authored
/// descriptor reference; core owns when the intro phase executes.
pub const ENGINE_STARTUP_INTRO_DESCRIPTOR_ENV: &str = "NEWENGINE_STARTUP_INTRO_DESCRIPTOR";
const ENGINE_RUNTIME_CONFIG_ENV: &str = "NEWENGINE_RUNTIME_CONFIG";
const ROOT_DIR_ENV: &str = "ROOT-DIR";
const LAUNCH_PROFILE_ENV: &str = "NEWENGINE_LAUNCH_PROFILE";
const RUNTIME_MODE_ENV: &str = "NEWENGINE_RUNTIME_MODE";
const GAME_MANIFEST_ENV: &str = "NEWENGINE_GAME_MANIFEST";

/// Core-owned game launch phase. The platform host calls this only after the
/// actual game window exists. The intro is rendered into that existing native
/// window and completes before engine plugin/world/scene loading begins.
/// Presentation failures are non-fatal and boot continues with an exact report.
pub fn present_in_game_window_if_configured(
    target: StartupIntroNativeWindow,
) -> Option<StartupIntroReport> {
    if env_bool("NEWENGINE_HEADLESS", false) || !game_launch_requested() {
        return None;
    }

    let raw_descriptor = env::var(ENGINE_STARTUP_INTRO_DESCRIPTOR_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())?;

    let runtime_config_path = env::var_os(ENGINE_RUNTIME_CONFIG_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("runtime.toml"));
    let root_dir = env::var_os(ROOT_DIR_ENV)
        .map(PathBuf::from)
        .or_else(|| runtime_config_path.parent().map(|path| path.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let descriptor = newengine_startup_intro::resolve_descriptor_path(
        &raw_descriptor,
        &runtime_config_path,
        &root_dir,
    );
    let report =
        newengine_startup_intro::play_from_descriptor_in_window(&descriptor, &root_dir, target);

    match report.status {
        StartupIntroStatus::Unavailable => eprintln!(
            "North Star core: game startup intro unavailable descriptor='{}': {}",
            report.descriptor_path.display(),
            report.detail
        ),
        StartupIntroStatus::Played => println!(
            "North Star core: game startup intro completed entries={} descriptor='{}'",
            report.entries,
            report.descriptor_path.display()
        ),
        _ => {}
    }

    Some(report)
}

fn game_launch_requested() -> bool {
    if let Ok(profile) = env::var(LAUNCH_PROFILE_ENV) {
        return profile.trim().eq_ignore_ascii_case("game");
    }

    env::var(RUNTIME_MODE_ENV)
        .ok()
        .is_some_and(|mode| mode.trim().eq_ignore_ascii_case("game"))
        && env::var_os(GAME_MANIFEST_ENV).is_some()
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn explicit_game_launch_profile_enables_intro_phase() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old = env::var_os(LAUNCH_PROFILE_ENV);
        env::set_var(LAUNCH_PROFILE_ENV, "game");
        assert!(game_launch_requested());
        restore_env(LAUNCH_PROFILE_ENV, old);
    }

    #[test]
    fn editor_launch_profile_does_not_enable_game_intro_phase() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old = env::var_os(LAUNCH_PROFILE_ENV);
        env::set_var(LAUNCH_PROFILE_ENV, "editor");
        assert!(!game_launch_requested());
        restore_env(LAUNCH_PROFILE_ENV, old);
    }

    fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }
}
