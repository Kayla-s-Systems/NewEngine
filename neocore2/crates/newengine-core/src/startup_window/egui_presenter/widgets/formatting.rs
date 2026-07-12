#![forbid(unsafe_op_in_unsafe_fn)]

use crate::startup_window::StartupLaunchSettings;

pub(in crate::startup_window::egui_presenter) fn mark_custom_if_changed(
    settings: &mut StartupLaunchSettings,
    changed: bool,
) {
    if changed {
        settings.graphics.mark_custom();
    }
}
pub(crate) fn aa_summary(settings: &StartupLaunchSettings) -> String {
    let mut parts = Vec::new();
    if settings.graphics.msaa_samples > 0 {
        parts.push(format!("{}× MSAA", settings.graphics.msaa_samples));
    }
    if settings.graphics.fxaa_enabled {
        parts.push("FXAA".to_owned());
    }
    if settings.graphics.taa_enabled {
        parts.push("TAA".to_owned());
    }
    if parts.is_empty() {
        "Off".to_owned()
    } else {
        parts.join(" + ")
    }
}
pub(in crate::startup_window::egui_presenter) fn format_msaa(samples: u8) -> String {
    if samples == 0 {
        "Off".to_owned()
    } else {
        format!("{samples}× MSAA")
    }
}
pub(in crate::startup_window::egui_presenter) fn format_anisotropy(value: u8) -> String {
    if value == 0 {
        "Off".to_owned()
    } else {
        format!("{value}×")
    }
}
pub(in crate::startup_window::egui_presenter) fn bool_string(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}
