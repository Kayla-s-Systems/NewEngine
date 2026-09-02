fn apply_graphics_process_overrides(cfg: &mut StartupConfig, report: &mut StartupLoadReport) {
    let overrides =
        apply_graphics_process_overrides_to_settings(&mut cfg.launch_settings, Some(report));
    if overrides > 0 {
        cfg.launch_settings_explicit = true;
    }
}

fn apply_graphics_process_overrides_to_settings(
    settings: &mut crate::startup_window::StartupLaunchSettings,
    mut report: Option<&mut StartupLoadReport>,
) -> usize {
    use crate::startup_window::{
        ENV_LOD_DISTANCE_SCALE, ENV_SHADOWS_ENABLED, ENV_SHADOW_CASCADE_COUNT,
        ENV_SHADOW_MAP_RESOLUTION,
    };

    let mut changed = 0usize;
    let mut record = |key: &'static str, from: String, to: String| {
        changed += 1;
        if let Some(report) = report.as_deref_mut() {
            report.overrides.push(StartupOverride { key, from, to });
        }
    };

    if let Some(raw) =
        newengine_plugin_host::current_host_context().environment_var(ENV_LOD_DISTANCE_SCALE)
    {
        if let Ok(value) = raw.trim().parse::<f32>() {
            let from = settings.graphics.lod_distance_scale.to_string();
            settings.graphics.lod_distance_scale = value;
            record(ENV_LOD_DISTANCE_SCALE, from, raw);
        }
    }
    if let Some(raw) =
        newengine_plugin_host::current_host_context().environment_var(ENV_SHADOWS_ENABLED)
    {
        if let Some(value) = parse_process_bool(&raw) {
            let from = settings.graphics.shadows_enabled.to_string();
            settings.graphics.shadows_enabled = value;
            record(ENV_SHADOWS_ENABLED, from, raw);
        }
    }
    if let Some(raw) =
        newengine_plugin_host::current_host_context().environment_var(ENV_SHADOW_CASCADE_COUNT)
    {
        if let Ok(value) = raw.trim().parse::<u32>() {
            let from = settings.graphics.shadow_cascade_count.to_string();
            settings.graphics.shadow_cascade_count = value;
            record(ENV_SHADOW_CASCADE_COUNT, from, raw);
        }
    }
    if let Some(raw) =
        newengine_plugin_host::current_host_context().environment_var(ENV_SHADOW_MAP_RESOLUTION)
    {
        if let Ok(value) = raw.trim().parse::<u32>() {
            let from = settings.graphics.shadow_map_resolution.to_string();
            settings.graphics.shadow_map_resolution = value;
            record(ENV_SHADOW_MAP_RESOLUTION, from, raw);
        }
    }
    if changed > 0 {
        settings.graphics.mark_custom();
        settings.normalize();
    }
    changed
}

fn parse_process_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
