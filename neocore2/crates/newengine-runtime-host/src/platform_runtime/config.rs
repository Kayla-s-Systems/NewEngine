use std::path::Path;

use abi_stable::std_types::{ROption, RString, RVec};
use libloading::Library;
use newengine_core::{EngineError, EngineResult, StartupConfig};
use newengine_platform_api::{
    PlatformAppConfigV1, PlatformRuntimeRunFnV1, PlatformWindowPlacementKindV1,
    PlatformWindowPlacementV1,
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
    }
}

#[inline]
fn config_patch_from_json_merge_patch(
    name: &str,
    priority: i32,
    value: &Value,
) -> ConfigPatchV1 {
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

    Ok(PlatformAppConfigV1 {
        title: title.into(),
        width,
        height,
        placement,
        icon: ROption::RNone,
    })
}

fn log_platform_config_diags(plugin_id: &str, diags: &[newengine_plugin_api::ConfigDiagV1]) {
    for diag in diags {
        match diag.level {
            ConfigDiagLevelV1::Info => log::info!(
                "platform runtime: config info id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Warn => log::warn!(
                "platform runtime: config warn id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Error => log::error!(
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
fn platform_metadata_probe_enabled() -> bool {
    std::env::var("NEWENGINE_PLATFORM_CONFIG_METADATA_PROBE")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
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

    config
}

fn resolve_platform_runtime_config_without_metadata_probe(
    startup: &StartupConfig,
    startup_defaults: PlatformAppConfigV1,
) -> ResolvedPlatformRuntimeConfig {
    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(PLATFORM_PLUGIN_ID);
    let icon_path = extract_string_field(&overrides, "icon")
        .or_else(|| startup.window_icon_path.clone());
    let config = apply_startup_platform_overrides(startup_defaults, &overrides);
    let descriptor = synthesize_platform_descriptor(
        PLATFORM_PLUGIN_ID,
        "NewEngine Platform Runtime",
        "-",
    );

    log::info!(
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
        plugin_version: "-".to_owned(),
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
        ));
    }

    crate::platform_early_log!(
        "host.config.metadata_probe.enabled runtime_path='{}'",
        runtime_path.display()
    );

    let lib = unsafe { Library::new(runtime_path) }
        .map_err(|e| EngineError::other(format!(
            "platform runtime metadata load failed: {e}"
        )))?;

    let root_sym = match unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL)
    } {
        Ok(sym) => sym,
        Err(_) => {
            log::info!(
                "platform runtime: plugin metadata not exported; using startup window config defaults"
            );
            let descriptor = synthesize_platform_descriptor(
                PLATFORM_PLUGIN_ID,
                "NewEngine Platform Runtime",
                "-",
            );
            return Ok(ResolvedPlatformRuntimeConfig {
                plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
                plugin_name: "NewEngine Platform Runtime".to_owned(),
                plugin_version: "-".to_owned(),
                descriptor,
                config: startup_defaults,
                icon_path: startup.window_icon_path.clone(),
            });
        }
    };

    let root = unsafe { root_sym() };
    let Some(create_v3) = root.create_v3() else {
        log::info!(
            "platform runtime: plugin metadata ABI V3 not available; using startup window config defaults"
        );
        let descriptor = synthesize_platform_descriptor(
            PLATFORM_PLUGIN_ID,
            "NewEngine Platform Runtime",
            "-",
        );
        return Ok(ResolvedPlatformRuntimeConfig {
            plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
            plugin_name: "NewEngine Platform Runtime".to_owned(),
            plugin_version: "-".to_owned(),
            descriptor,
            config: startup_defaults,
            icon_path: startup.window_icon_path.clone(),
        });
    };

    let module = create_v3();
    let descriptor = ensure_platform_runtime_capabilities(module.descriptor_v3());
    let plugin_id = descriptor.id.to_string();
    let plugin_name = descriptor.name.to_string();
    let plugin_version = descriptor.version.to_string();

    let defaults = module
        .config_defaults_v1()
        .into_result()
        .map_err(|e| EngineError::other(format!(
            "platform config defaults failed: {e}"
        )))?;

    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(&plugin_id);
    let icon_path = extract_string_field(&overrides, "icon");
    let plugin_patch = strip_host_only_platform_keys(&overrides);

    let mut patches = RVec::<ConfigPatchV1>::new();
    if is_non_empty_object(&plugin_patch) {
        patches.push(config_patch_from_json_merge_patch("config+env", 0, &plugin_patch));
    }

    let applied = module
        .config_apply_patches_v1(&defaults, patches)
        .into_result()
        .map_err(|e| EngineError::other(format!(
            "platform config apply failed: {e}"
        )))?;

    log_platform_config_diags(&plugin_id, applied.diags.as_slice());

    let config = platform_config_from_effective_blob(&applied.effective)
        .map_err(|e| EngineError::other(format!(
            "platform config decode failed: {e}"
        )))?;

    log::info!(
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

fn ensure_platform_runtime_capabilities(mut descriptor: PluginDescriptor) -> PluginDescriptor {
    fn has_cap(
        descriptor: &PluginDescriptor,
        id: &str,
        role: CapabilityRole,
        kind: CapabilityKind,
        version: u32,
    ) -> bool {
        descriptor.capabilities.iter().any(|cap| {
            cap.id.as_str() == id
                && cap.role == role
                && cap.kind == kind
                && cap.version >= version
        })
    }

    let required = [
        (
            "platform.runtime.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
            r#"{"role":"platform-runtime"}"#,
        ),
        (
            "platform.window.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
            r#"{"role":"window"}"#,
        ),
        (
            "platform.window.v1",
            CapabilityRole::Provides,
            CapabilityKind::ServiceV1,
            1,
            r#"{"role":"platform-window-snapshot"}"#,
        ),
        (
            "platform.surface.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
            r#"{"role":"surface"}"#,
        ),
        (
            "platform.input.events.v1",
            CapabilityRole::Provides,
            CapabilityKind::EventsV1,
            1,
            r#"{"role":"input-events"}"#,
        ),
    ];

    for (id, role, kind, version, json) in required {
        if !has_cap(&descriptor, id, role, kind, version) {
            descriptor.capabilities.push(
                CapabilityDesc::new(id, role, kind, version).with_json(json),
            );
        }
    }

    descriptor
}

fn synthesize_platform_descriptor(
    id: &str,
    name: &str,
    version: &str,
) -> PluginDescriptor {
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
                "platform.window.v1",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
                .with_json(r#"{"role":"window"}"#),
        )
        .provides_service(
            "platform.window.v1",
            1,
            r#"{"role":"platform-window-snapshot"}"#,
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