use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use newengine_ui_api::UiEventDispatchFrame;

const FRONTEND_SETTINGS_SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

fn frontend_settings_pending() -> &'static Mutex<BTreeMap<String, serde_json::Value>> {
    static PENDING: OnceLock<Mutex<BTreeMap<String, serde_json::Value>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn frontend_settings_last_changed() -> &'static Mutex<Option<Instant>> {
    static LAST_CHANGED: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LAST_CHANGED.get_or_init(|| Mutex::new(None))
}

fn mark_frontend_settings_changed() {
    *frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
}

pub(super) fn frontend_settings_debounce_due() -> bool {
    frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_some_and(|changed_at| changed_at.elapsed() >= FRONTEND_SETTINGS_SAVE_DEBOUNCE)
}

fn clear_frontend_settings_changed_at() {
    *frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn lock_frontend_settings_pending(
) -> std::sync::MutexGuard<'static, BTreeMap<String, serde_json::Value>> {
    frontend_settings_pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) fn stage_frontend_setting_actions(frame: &UiEventDispatchFrame) {
    let mut changed = false;
    let mut pending = lock_frontend_settings_pending();
    for action in &frame.actions {
        if action.trigger != newengine_ui_api::UiNodeEventTrigger::ValueChanged
            || !action.action_id.starts_with("settings.")
        {
            continue;
        }
        let Some(value) = action.payload.get("value") else {
            continue;
        };
        pending.insert(action.action_id.clone(), value.clone());
        changed = true;
    }
    drop(pending);
    if changed {
        mark_frontend_settings_changed();
    }
}

pub(super) fn frontend_settings_apply_requested(frame: &UiEventDispatchFrame) -> bool {
    frame.actions.iter().any(|action| {
        action.trigger == newengine_ui_api::UiNodeEventTrigger::Click
            && action.action_id == "settings.apply"
    })
}

pub(super) fn persist_frontend_settings() -> Result<usize, String> {
    let changes = {
        let mut pending = lock_frontend_settings_pending();
        std::mem::take(&mut *pending)
    };
    clear_frontend_settings_changed_at();
    if changes.is_empty() {
        return Ok(0);
    }

    let config_path = std::env::current_dir()
        .map_err(|error| format!("resolve current directory: {error}"))?
        .join("config.json");
    let source = std::fs::read_to_string(&config_path)
        .map_err(|error| format!("read '{}': {error}", config_path.display()))?;
    let mut document: serde_json::Value = serde_json::from_str(&source)
        .map_err(|error| format!("parse '{}': {error}", config_path.display()))?;

    let mut applied = 0usize;
    for (action_id, value) in changes {
        if apply_frontend_setting_value(&mut document, action_id.as_str(), &value) {
            applied += 1;
        }
    }
    if applied == 0 {
        return Ok(0);
    }
    let encoded = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("encode settings config: {error}"))?;
    std::fs::write(&config_path, format!("{encoded}\n"))
        .map_err(|error| format!("write '{}': {error}", config_path.display()))?;
    Ok(applied)
}

pub(super) fn apply_frontend_setting_value(
    document: &mut serde_json::Value,
    action_id: &str,
    value: &serde_json::Value,
) -> bool {
    match action_id {
        "settings.display.fullscreen" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            let mode = serde_json::Value::String(
                if enabled {
                    "exclusive_fullscreen"
                } else {
                    "windowed"
                }
                .to_owned(),
            );
            set_json_pointer(
                document,
                "/startup_settings/display/window_mode",
                mode.clone(),
            );
            set_json_pointer(document, "/window/display/window_mode", mode.clone());
            set_json_pointer(
                document,
                "/plugins/newengine/startup_window/display/window_mode",
                mode.clone(),
            );
            set_json_pointer(
                document,
                "/plugins/newengine/startup_window/display/fullscreen",
                serde_json::json!(enabled),
            );
            set_json_pointer(
                document,
                "/plugins/engine.platform.winit/display/window_mode",
                mode,
            );
            set_json_pointer(
                document,
                "/plugins/engine.platform.winit/display/fullscreen",
                serde_json::json!(enabled),
            );
            true
        }
        "settings.display.vsync" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            for pointer in [
                "/startup_settings/display/vsync",
                "/window/display/vsync",
                "/plugins/newengine/startup_window/display/vsync",
                "/plugins/engine.platform.winit/display/vsync",
            ] {
                set_json_pointer(document, pointer, serde_json::json!(enabled));
            }
            true
        }
        "settings.display.render_scale" => {
            let Some(scale) = json_setting_f64(value).map(|value| value.clamp(0.5, 1.5)) else {
                return false;
            };
            for pointer in [
                "/startup_settings/display/render_scale",
                "/window/display/render_scale",
                "/plugins/newengine/startup_window/display/render_scale",
                "/plugins/engine.platform.winit/display/render_scale",
            ] {
                set_json_pointer(document, pointer, serde_json::json!(scale));
            }
            true
        }
        "settings.graphics.bloom"
        | "settings.graphics.motion_blur"
        | "settings.graphics.depth_of_field"
        | "settings.graphics.sun_rays"
        | "settings.graphics.shadows" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            let field = match action_id {
                "settings.graphics.bloom" => "bloom_enabled",
                "settings.graphics.motion_blur" => "motion_blur_enabled",
                "settings.graphics.depth_of_field" => "depth_of_field_enabled",
                "settings.graphics.sun_rays" => "sun_rays_enabled",
                "settings.graphics.shadows" => "shadows_enabled",
                _ => unreachable!(),
            };
            set_json_pointer(
                document,
                format!("/startup_settings/graphics/{field}").as_str(),
                serde_json::json!(enabled),
            );
            set_json_pointer(
                document,
                "/startup_settings/graphics/preset",
                serde_json::Value::String("custom".to_owned()),
            );
            true
        }
        _ => false,
    }
}

fn set_json_pointer(document: &mut serde_json::Value, pointer: &str, value: serde_json::Value) {
    if let Some(slot) = document.pointer_mut(pointer) {
        *slot = value;
    }
}

fn json_setting_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        value
            .as_str()
            .and_then(|text| match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "on" | "yes" | "checked" | "selected" => Some(true),
                "false" | "0" | "off" | "no" | "unchecked" | "unselected" => Some(false),
                _ => None,
            })
    })
}

fn json_setting_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
}
