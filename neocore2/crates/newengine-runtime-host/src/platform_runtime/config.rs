use std::path::Path;

use abi_stable::std_types::{ROption, RString, RVec};
use libloading::Library;
use newengine_core::{EngineError, EngineResult, StartupConfig};
use newengine_platform_api::{
    PlatformAppConfigV1, PlatformDisplayConfigV1, PlatformHdrModeV1, PlatformRuntimeRunFnV1,
    PlatformWindowModeV1, PlatformWindowPlacementKindV1, PlatformWindowPlacementV1,
};
use newengine_plugin_api::{
    CapabilityDesc, CapabilityKind, CapabilityRole, ConfigBlobV1, ConfigDiagLevelV1,
    ConfigPatchSourceV1, ConfigPatchV1, PluginDescriptor, PluginKind, PluginRootV1Ref,
};
use serde_json::Value;

use crate::platform_runtime::constants::{
    CT_JSON_MERGE_PATCH, PLATFORM_PLUGIN_ID, PLUGIN_ROOT_SYMBOL,
};
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;

#[inline]
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

    PlatformAppConfigV1 {
        title: startup.window_title.clone().into(),
        width: startup.window_size.0,
        height: startup.window_size.1,
        placement,
        icon: ROption::RNone,
        display: PlatformDisplayConfigV1::default(),
    }
}

#[inline]
fn config_patch_from_json_merge_patch(name: &str, priority: i32, value: &Value) -> ConfigPatchV1 {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    ConfigPatchV1 {
        source: ConfigPatchSourceV1::HostRule,
        content_type: RString::from(CT_JSON_MERGE_PATCH),
        bytes: RVec::from(bytes),
        priority,
        name: RString::from(name),
    }
}

#[inline]
fn is_non_empty_object(value: &Value) -> bool {
    matches!(value, Value::Object(map) if !map.is_empty())
}

fn platform_config_from_effective_blob(blob: &ConfigBlobV1) -> Result<PlatformAppConfigV1, String> {
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

fn log_platform_config_diags(plugin_id: &str, diags: &[newengine_plugin_api::ConfigDiagV1]) {
    for diag in diags {
        match diag.level {
            ConfigDiagLevelV1::Info => newengine_ulog_api::ulog::info!(
                "platform runtime: config info id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Warn => newengine_ulog_api::ulog::warn!(
                "platform runtime: config warn id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Error => newengine_ulog_api::ulog::error!(
                "platform runtime: config error id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
        }
    }
}

fn extract_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
}

fn strip_host_only_platform_keys(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("icon");
    }
    value
}

#[inline]
fn env_flag(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[inline]
pub(crate) fn runtime_bootstrap_overlay_enabled() -> bool {
    !env_flag("NEWENGINE_RUNTIME_BOOTSTRAP_OVERLAY_DISABLED").unwrap_or(false)
}

#[inline]
pub(crate) fn game_screen_diagnostic_panel_enabled() -> bool {
    env_flag("NEWENGINE_GAME_SCREEN_DIAGNOSTIC_PANEL").unwrap_or(false)
}

#[inline]
fn platform_metadata_probe_enabled() -> bool {
    std::env::var("NEWENGINE_PLATFORM_CONFIG_METADATA_PROBE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn apply_startup_platform_overrides(
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

    let vsync = obj.get("vsync").and_then(Value::as_bool).unwrap_or(true);

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

fn resolve_platform_runtime_config_without_metadata_probe(
    startup: &StartupConfig,
    startup_defaults: PlatformAppConfigV1,
    runtime_path: &Path,
) -> ResolvedPlatformRuntimeConfig {
    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(PLATFORM_PLUGIN_ID);
    let icon_path =
        extract_string_field(&overrides, "icon").or_else(|| startup.window_icon_path.clone());
    let config = apply_startup_platform_overrides(startup_defaults, &overrides);
    let plugin_version = platform_runtime_version_from_path(runtime_path);
    let descriptor = synthesize_platform_descriptor(
        PLATFORM_PLUGIN_ID,
        "NewEngine Platform Runtime",
        &plugin_version,
    );

    newengine_ulog_api::ulog::info!(
        "platform runtime: metadata probe disabled; using host-side config id='{}' title='{}' size={}x{} placement={:?} icon={}",
        PLATFORM_PLUGIN_ID,
        config.title,
        config.width,
        config.height,
        config.placement.kind,
        icon_path.as_deref().unwrap_or("<none>")
    );

    ResolvedPlatformRuntimeConfig {
        plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
        plugin_name: "NewEngine Platform Runtime".to_owned(),
        plugin_version,
        descriptor,
        config,
        icon_path,
    }
}

pub fn resolve_platform_runtime_config(
    startup: &StartupConfig,
    runtime_path: &Path,
) -> EngineResult<ResolvedPlatformRuntimeConfig> {
    let startup_defaults = platform_config_from_startup_defaults(startup);

    if !platform_metadata_probe_enabled() {
        crate::platform_early_log!(
            "host.config.metadata_probe.disabled runtime_path='{}'",
            runtime_path.display()
        );
        return Ok(resolve_platform_runtime_config_without_metadata_probe(
            startup,
            startup_defaults,
            runtime_path,
        ));
    }

    crate::platform_early_log!(
        "host.config.metadata_probe.enabled runtime_path='{}'",
        runtime_path.display()
    );

    let lib = unsafe { Library::new(runtime_path) }
        .map_err(|e| EngineError::other(format!("platform runtime metadata load failed: {e}")))?;

    let root_sym = match unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL)
    } {
        Ok(sym) => sym,
        Err(_) => {
            newengine_ulog_api::ulog::info!(
                "platform runtime: plugin metadata not exported; using startup window config defaults"
            );
            let plugin_version = platform_runtime_version_from_path(runtime_path);
            let descriptor = synthesize_platform_descriptor(
                PLATFORM_PLUGIN_ID,
                "NewEngine Platform Runtime",
                &plugin_version,
            );
            return Ok(ResolvedPlatformRuntimeConfig {
                plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
                plugin_name: "NewEngine Platform Runtime".to_owned(),
                plugin_version,
                descriptor,
                config: startup_defaults,
                icon_path: startup.window_icon_path.clone(),
            });
        }
    };

    let root = unsafe { root_sym() };
    let module = root.create()();
    let descriptor = ensure_platform_runtime_capabilities(module.descriptor());
    let plugin_id = descriptor.id.to_string();
    let plugin_name = descriptor.name.to_string();
    let plugin_version = descriptor.version.to_string();

    let defaults = module
        .config_defaults()
        .into_result()
        .map_err(|e| EngineError::other(format!("platform config defaults failed: {e}")))?;

    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(&plugin_id);
    let icon_path = extract_string_field(&overrides, "icon");
    let plugin_patch = strip_host_only_platform_keys(&overrides);

    let mut patches = RVec::<ConfigPatchV1>::new();
    if is_non_empty_object(&plugin_patch) {
        patches.push(config_patch_from_json_merge_patch(
            "config+env",
            0,
            &plugin_patch,
        ));
    }

    let applied = module
        .config_apply_patches(&defaults, patches)
        .into_result()
        .map_err(|e| EngineError::other(format!("platform config apply failed: {e}")))?;

    log_platform_config_diags(&plugin_id, applied.diags.as_slice());

    let config = platform_config_from_effective_blob(&applied.effective)
        .map_err(|e| EngineError::other(format!("platform config decode failed: {e}")))?;

    newengine_ulog_api::ulog::info!(
        "platform runtime: effective config id='{}' title='{}' size={}x{} placement={:?} icon={}",
        plugin_id,
        config.title,
        config.width,
        config.height,
        config.placement.kind,
        icon_path.as_deref().unwrap_or("<none>")
    );

    Ok(ResolvedPlatformRuntimeConfig {
        plugin_id,
        plugin_name,
        plugin_version,
        descriptor,
        config,
        icon_path,
    })
}

fn platform_runtime_version_from_path(runtime_path: &Path) -> String {
    let Some(stem) = runtime_path.file_stem().and_then(|stem| stem.to_str()) else {
        return "-".to_owned();
    };

    stem.split('-')
        .find(|part| looks_like_semver(part))
        .map(str::to_owned)
        .unwrap_or_else(|| "-".to_owned())
}

fn looks_like_semver(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(major) = segments.next() else {
        return false;
    };
    let Some(minor) = segments.next() else {
        return false;
    };
    let Some(patch) = segments.next() else {
        return false;
    };
    segments.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
        && minor.chars().all(|ch| ch.is_ascii_digit())
        && patch.chars().all(|ch| ch.is_ascii_digit())
}

fn ensure_platform_runtime_capabilities(mut descriptor: PluginDescriptor) -> PluginDescriptor {
    fn has_cap(
        descriptor: &PluginDescriptor,
        id: &str,
        role: CapabilityRole,
        kind: CapabilityKind,
        version: u32,
    ) -> bool {
        descriptor.capabilities.iter().any(|cap| {
            cap.id.as_str() == id && cap.role == role && cap.kind == kind && cap.version >= version
        })
    }

    // The platform runtime is an external event-loop entrypoint, not a plugin-owned
    // `platform.api` ServiceV1 provider. `engine.platform` is registered later as an
    // engine-runtime snapshot gateway by `snapshot_service.rs`; advertising a backend
    // route here makes the gateway registry try to bind `platform.api` every frame.
    let required = vec![
        CapabilityDesc::new(
            "platform.runtime.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json(r#"{"role":"platform-runtime"}"#),
        CapabilityDesc::new(
            "platform.surface.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json(r#"{"role":"surface"}"#),
        CapabilityDesc::new(
            "platform.input.events.v1",
            CapabilityRole::Provides,
            CapabilityKind::EventsV1,
            1,
        )
        .with_json(r#"{"role":"input-events"}"#),
    ];

    for cap in required {
        if !has_cap(
            &descriptor,
            cap.id.as_str(),
            cap.role,
            cap.kind,
            cap.version,
        ) {
            descriptor.capabilities.push(cap);
        }
    }

    descriptor
}

fn synthesize_platform_descriptor(id: &str, name: &str, version: &str) -> PluginDescriptor {
    PluginDescriptor::builder(id, name, version, PluginKind::Runtime)
        .push(
            CapabilityDesc::new(
                "platform.runtime.v1",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"role":"platform-runtime"}"#),
        )
        .push(
            CapabilityDesc::new(
                "platform.surface.v1",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"role":"surface"}"#),
        )
        .push(
            CapabilityDesc::new(
                "platform.input.events.v1",
                CapabilityRole::Provides,
                CapabilityKind::EventsV1,
                1,
            )
            .with_json(r#"{"role":"input-events"}"#),
        )
        .build()
}

#[allow(dead_code)]
fn _abi_marker(_: PlatformRuntimeRunFnV1) {}
