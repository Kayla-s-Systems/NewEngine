use abi_stable::std_types::{ROption, RString};
use newengine_core::StartupConfig;
use newengine_platform_api::{
    PlatformAppConfigV1, PlatformDisplayConfigV1, PlatformHdrModeV1, PlatformWindowModeV1,
    PlatformWindowPlacementKindV1, PlatformWindowPlacementV1,
};
use newengine_plugin_api::ConfigBlobV1;
use serde_json::Value;

pub fn platform_config_from_startup_defaults(startup: &StartupConfig) -> PlatformAppConfigV1 {
    let placement = match startup.window_placement {
        newengine_core::startup::WindowPlacement::Default => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::OsDefault,
            x: 0,
            y: 0,
        },
        newengine_core::startup::WindowPlacement::Centered { offset } => {
            PlatformWindowPlacementV1 {
                kind: PlatformWindowPlacementKindV1::Centered,
                x: offset.0,
                y: offset.1,
            }
        }
    };

    let display = if startup.launch_settings_explicit {
        platform_display_from_launch_settings(startup)
    } else {
        PlatformDisplayConfigV1::default()
    };
    let (width, height) = if startup.launch_settings_explicit {
        match startup.launch_settings.display.resolution {
            [width, height] if width > 0 && height > 0 => {
                (width.clamp(64, 16_384), height.clamp(64, 16_384))
            }
            _ => startup.window_size,
        }
    } else {
        startup.window_size
    };

    PlatformAppConfigV1 {
        title: startup.window_title.clone().into(),
        width,
        height,
        placement,
        icon: ROption::RNone,
        display,
    }
}

fn platform_display_from_launch_settings(startup: &StartupConfig) -> PlatformDisplayConfigV1 {
    let launch_display = &startup.launch_settings.display;
    let window_mode = match launch_display.window_mode {
        newengine_core::StartupWindowMode::Windowed => PlatformWindowModeV1::Windowed,
        newengine_core::StartupWindowMode::Borderless => PlatformWindowModeV1::Borderless,
        newengine_core::StartupWindowMode::ExclusiveFullscreen => {
            PlatformWindowModeV1::ExclusiveFullscreen
        }
    };
    let hdr = match launch_display.hdr {
        newengine_core::StartupHdrMode::Auto => PlatformHdrModeV1::Auto,
        newengine_core::StartupHdrMode::Enabled => PlatformHdrModeV1::Enabled,
        newengine_core::StartupHdrMode::Disabled => PlatformHdrModeV1::Disabled,
    };
    PlatformDisplayConfigV1 {
        monitor_index: launch_display.monitor_index,
        window_mode,
        vsync: launch_display.vsync,
        refresh_rate_millihz: launch_display.refresh_rate_millihz,
        render_scale: launch_display.render_scale,
        hdr,
    }
}

pub(super) fn apply_confirmed_core_launch_settings(
    mut config: PlatformAppConfigV1,
    startup: &StartupConfig,
) -> PlatformAppConfigV1 {
    if !startup.launch_settings_explicit {
        return config;
    }

    let confirmed = platform_config_from_startup_defaults(startup);
    config.title = confirmed.title;
    config.width = confirmed.width;
    config.height = confirmed.height;
    config.placement = confirmed.placement;
    config.display = platform_display_from_launch_settings(startup);
    config
}
pub(super) fn platform_config_from_effective_blob(
    blob: &ConfigBlobV1,
) -> Result<PlatformAppConfigV1, String> {
    if blob.content_type.as_str() != "application/json" {
        return Err(format!(
            "unsupported platform config content_type '{}'",
            blob.content_type
        ));
    }

    let value: Value = serde_json::from_slice(blob.bytes.as_slice())
        .map_err(|e| format!("platform config parse failed: {e}"))?;

    let obj = value
        .as_object()
        .ok_or_else(|| "platform config must be a JSON object".to_owned())?;

    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("NewEngine")
        .to_owned();

    let width = obj
        .get("width")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(1600)
        .clamp(64, 16384);

    let height = obj
        .get("height")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(900)
        .clamp(64, 16384);

    let placement_obj = obj.get("placement").and_then(Value::as_object);
    let placement_mode = placement_obj
        .and_then(|it| it.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("os_default");

    let placement = match placement_mode {
        "os_default" | "default" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::OsDefault,
            x: 0,
            y: 0,
        },
        "centered" | "center" | "centre" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::Centered,
            x: placement_obj
                .and_then(|it| it.get("x"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
            y: placement_obj
                .and_then(|it| it.get("y"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
        },
        "absolute" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::Absolute,
            x: placement_obj
                .and_then(|it| it.get("x"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
            y: placement_obj
                .and_then(|it| it.get("y"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
        },
        other => return Err(format!("unsupported placement.mode '{other}'")),
    };

    let display = parse_display_config(obj.get("display"));

    Ok(PlatformAppConfigV1 {
        title: title.into(),
        width,
        height,
        placement,
        icon: ROption::RNone,
        display,
    })
}
pub(super) fn apply_startup_platform_overrides(
    mut config: PlatformAppConfigV1,
    overrides: &Value,
) -> PlatformAppConfigV1 {
    let Some(obj) = overrides.as_object() else {
        return config;
    };

    if let Some(title) = obj
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        config.title = RString::from(title);
    }

    if let Some(width) = obj.get("width").and_then(Value::as_u64) {
        config.width = (width as u32).clamp(64, 16384);
    }

    if let Some(height) = obj.get("height").and_then(Value::as_u64) {
        config.height = (height as u32).clamp(64, 16384);
    }

    if let Some(placement_obj) = obj.get("placement").and_then(Value::as_object) {
        let mode = placement_obj
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("os_default");
        config.placement = match mode {
            "centered" | "center" | "centre" => PlatformWindowPlacementV1 {
                kind: PlatformWindowPlacementKindV1::Centered,
                x: placement_obj
                    .get("x")
                    .and_then(Value::as_i64)
                    .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                    .unwrap_or(0),
                y: placement_obj
                    .get("y")
                    .and_then(Value::as_i64)
                    .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                    .unwrap_or(0),
            },
            "absolute" => PlatformWindowPlacementV1 {
                kind: PlatformWindowPlacementKindV1::Absolute,
                x: placement_obj
                    .get("x")
                    .and_then(Value::as_i64)
                    .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                    .unwrap_or(0),
                y: placement_obj
                    .get("y")
                    .and_then(Value::as_i64)
                    .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                    .unwrap_or(0),
            },
            _ => PlatformWindowPlacementV1 {
                kind: PlatformWindowPlacementKindV1::OsDefault,
                x: 0,
                y: 0,
            },
        };
    }

    if let Some(display_obj) = obj.get("display") {
        config.display = parse_display_config(Some(display_obj));
    }

    config
}

fn parse_display_config(value: Option<&Value>) -> PlatformDisplayConfigV1 {
    let Some(obj) = value.and_then(Value::as_object) else {
        return PlatformDisplayConfigV1::default();
    };

    let monitor_index = obj
        .get("monitor_index")
        .and_then(Value::as_i64)
        .map(|v| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
        .or_else(|| {
            obj.get("monitor")
                .and_then(Value::as_str)
                .map(parse_monitor_index)
        })
        .unwrap_or(-1);

    let window_mode = obj
        .get("window_mode")
        .and_then(Value::as_str)
        .map(parse_window_mode)
        .unwrap_or(PlatformWindowModeV1::Windowed);

    let vsync = obj.get("vsync").and_then(Value::as_bool).unwrap_or(false);

    let refresh_rate_millihz = obj
        .get("refresh_rate_millihz")
        .and_then(Value::as_u64)
        .map(|v| v.min(u64::from(u32::MAX)) as u32)
        .or_else(|| parse_refresh_rate_millihz(obj.get("refresh_rate")))
        .unwrap_or(0);

    let render_scale = obj
        .get("render_scale")
        .and_then(Value::as_f64)
        .map(|v| (v as f32).clamp(0.25, 2.0))
        .unwrap_or(1.0);

    let hdr = obj
        .get("hdr")
        .and_then(Value::as_str)
        .map(parse_hdr_mode)
        .unwrap_or(PlatformHdrModeV1::Auto);

    PlatformDisplayConfigV1 {
        monitor_index,
        window_mode,
        vsync,
        refresh_rate_millihz,
        render_scale,
        hdr,
    }
}

fn parse_monitor_index(value: &str) -> i32 {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("primary")
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.is_empty()
    {
        return -1;
    }
    trimmed.parse::<i32>().unwrap_or(-1)
}

fn parse_window_mode(value: &str) -> PlatformWindowModeV1 {
    match value.trim().to_ascii_lowercase().as_str() {
        "borderless" | "borderless_fullscreen" => PlatformWindowModeV1::Borderless,
        "exclusive" | "exclusive_fullscreen" | "fullscreen" => {
            PlatformWindowModeV1::ExclusiveFullscreen
        }
        _ => PlatformWindowModeV1::Windowed,
    }
}

fn parse_hdr_mode(value: &str) -> PlatformHdrModeV1 {
    match value.trim().to_ascii_lowercase().as_str() {
        "enabled" | "on" | "true" => PlatformHdrModeV1::Enabled,
        "disabled" | "off" | "false" => PlatformHdrModeV1::Disabled,
        _ => PlatformHdrModeV1::Auto,
    }
}

fn parse_refresh_rate_millihz(value: Option<&Value>) -> Option<u32> {
    match value? {
        Value::String(s) => {
            let t = s.trim().to_ascii_lowercase();
            if t == "auto" {
                return Some(0);
            }
            t.trim_end_matches("hz")
                .parse::<u32>()
                .ok()
                .map(|hz| hz.saturating_mul(1000))
        }
        Value::Number(n) => n
            .as_u64()
            .map(|hz| (hz.min(u64::from(u32::MAX / 1000)) as u32).saturating_mul(1000)),
        _ => None,
    }
}
