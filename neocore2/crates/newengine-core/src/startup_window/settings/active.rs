static ACTIVE_SETTINGS: OnceLock<RwLock<StartupLaunchSettings>> = OnceLock::new();

pub fn startup_launch_settings() -> StartupLaunchSettings {
    ACTIVE_SETTINGS
        .get_or_init(|| RwLock::new(StartupLaunchSettings::default()))
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn set_startup_launch_settings(mut settings: StartupLaunchSettings) {
    settings.normalize();
    settings.publish_environment_snapshot();
    let lock = ACTIVE_SETTINGS.get_or_init(|| RwLock::new(StartupLaunchSettings::default()));
    *lock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
}

#[inline]
fn normalize_shadow_map_resolution(value: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    match value {
        0..=256 => 256,
        257..=512 => 512,
        513..=1024 => 1024,
        1025..=2048 => 2048,
        2049..=4096 => 4096,
        4097..=8192 => 8192,
        _ => 16284,
    }
}

#[inline]
fn bool_text(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

#[inline]
fn set_env(key: &str, value: impl AsRef<str>) {
    newengine_plugin_host::current_host_context().set_environment_var(key, value.as_ref());
}
