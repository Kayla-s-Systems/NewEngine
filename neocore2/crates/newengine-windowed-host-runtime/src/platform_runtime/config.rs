use std::path::Path;

use abi_stable::std_types::RVec;
use libloading::Library;
use newengine_core::{EngineError, EngineResult, StartupConfig};
use newengine_platform_api::PlatformAppConfigV1;
use newengine_plugin_api::{ConfigPatchV1, PluginRootV1Ref};

use crate::platform_runtime::constants::PLUGIN_ROOT_SYMBOL;
use crate::platform_runtime::discovery::try_read_runtime_descriptor;
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;

mod descriptor;
mod display;
mod patches;

pub use display::platform_config_from_startup_defaults;
pub(crate) use patches::runtime_bootstrap_overlay_enabled;

use descriptor::{
    ensure_platform_runtime_capabilities, platform_runtime_version_from_path,
    synthesize_platform_descriptor,
};
use display::{
    apply_confirmed_core_launch_settings, apply_startup_platform_overrides,
    platform_config_from_effective_blob,
};
use patches::{
    config_patch_from_json_merge_patch, extract_string_field, is_non_empty_object,
    log_platform_config_diags, platform_metadata_probe_enabled, strip_host_only_platform_keys,
};

const GENERIC_PLATFORM_RUNTIME_ID: &str = "engine.platform.runtime";
const GENERIC_PLATFORM_RUNTIME_NAME: &str = "NewEngine Platform Runtime";

fn runtime_identity(runtime_path: &Path) -> (String, String, String) {
    if let Some(descriptor) = try_read_runtime_descriptor(runtime_path) {
        let id = descriptor.id.to_string();
        let name = descriptor.name.to_string();
        let version = descriptor.version.to_string();
        return (
            if id.trim().is_empty() {
                GENERIC_PLATFORM_RUNTIME_ID.to_owned()
            } else {
                id
            },
            if name.trim().is_empty() {
                GENERIC_PLATFORM_RUNTIME_NAME.to_owned()
            } else {
                name
            },
            if version.trim().is_empty() {
                platform_runtime_version_from_path(runtime_path)
            } else {
                version
            },
        );
    }
    (
        GENERIC_PLATFORM_RUNTIME_ID.to_owned(),
        GENERIC_PLATFORM_RUNTIME_NAME.to_owned(),
        platform_runtime_version_from_path(runtime_path),
    )
}

fn resolve_platform_runtime_config_without_metadata_probe(
    startup: &StartupConfig,
    startup_defaults: PlatformAppConfigV1,
    runtime_path: &Path,
) -> ResolvedPlatformRuntimeConfig {
    let (plugin_id, plugin_name, plugin_version) = runtime_identity(runtime_path);
    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(&plugin_id);
    let icon_path =
        extract_string_field(&overrides, "icon").or_else(|| startup.window_icon_path.clone());
    let config = apply_confirmed_core_launch_settings(
        apply_startup_platform_overrides(startup_defaults, &overrides),
        startup,
    );
    let descriptor = synthesize_platform_descriptor(&plugin_id, &plugin_name, &plugin_version);

    newengine_ulog_api::ulog::info!(
        "platform runtime: metadata probe disabled; using descriptor identity id='{}' title='{}' size={}x{} placement={:?} icon={}",
        plugin_id,
        config.title,
        config.width,
        config.height,
        config.placement.kind,
        icon_path.as_deref().unwrap_or("<none>")
    );

    ResolvedPlatformRuntimeConfig {
        plugin_id,
        plugin_name,
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
            let (plugin_id, plugin_name, plugin_version) = runtime_identity(runtime_path);
            let descriptor =
                synthesize_platform_descriptor(&plugin_id, &plugin_name, &plugin_version);
            return Ok(ResolvedPlatformRuntimeConfig {
                plugin_id,
                plugin_name,
                plugin_version,
                descriptor,
                config: startup_defaults,
                icon_path: startup.window_icon_path.clone(),
            });
        }
    };

    let root = unsafe { root_sym() };
    let module = root.create()();
    let raw_descriptor = module.descriptor();
    let manifest = newengine_plugin_host::read_verified_plugin_discovery_manifest(runtime_path)
        .map_err(|e| {
            EngineError::other(format!(
                "platform runtime discovery metadata verification failed: {e}"
            ))
        })?;
    let planned_descriptor = manifest.descriptor.ok_or_else(|| {
        EngineError::other(format!(
            "platform runtime sidecar has no plugin descriptor for '{}'",
            runtime_path.display()
        ))
    })?;
    let live_descriptor_v2 = newengine_plugin_api::PluginDescriptorV2::from_legacy(&raw_descriptor);
    let live_projection =
        newengine_plugin_api::PluginDiscoveryDescriptorV1::from_descriptor_v2(&live_descriptor_v2);
    if planned_descriptor != live_projection {
        return Err(EngineError::other(format!(
            "platform runtime plugin descriptor does not match frozen discovery metadata path='{}'",
            runtime_path.display()
        )));
    }
    let descriptor = ensure_platform_runtime_capabilities(raw_descriptor);
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

    let config = apply_confirmed_core_launch_settings(
        platform_config_from_effective_blob(&applied.effective)
            .map_err(|e| EngineError::other(format!("platform config decode failed: {e}")))?,
        startup,
    );

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
#[cfg(test)]
mod tests {
    use super::*;
    use newengine_platform_api::{PlatformHdrModeV1, PlatformWindowModeV1};

    #[test]
    fn unrequested_launch_settings_do_not_change_historical_platform_defaults() {
        let mut startup = StartupConfig::default();
        startup.launch_settings.display.window_mode = newengine_core::StartupWindowMode::Borderless;
        startup.launch_settings.display.vsync = false;
        startup.launch_settings.display.render_scale = 1.5;
        startup.launch_settings_explicit = false;

        let config = platform_config_from_startup_defaults(&startup);

        assert_eq!(config.display.window_mode, PlatformWindowModeV1::Windowed);
        assert!(config.display.vsync);
        assert_eq!(config.display.render_scale, 1.0);
    }

    #[test]
    fn explicit_core_launch_settings_feed_platform_creation() {
        let mut startup = StartupConfig {
            window_size: (2560, 1440),
            ..StartupConfig::default()
        };
        startup.launch_settings.display.monitor_index = 2;
        startup.launch_settings.display.window_mode =
            newengine_core::StartupWindowMode::ExclusiveFullscreen;
        startup.launch_settings.display.vsync = false;
        startup.launch_settings.display.refresh_rate_millihz = 165_000;
        startup.launch_settings.display.render_scale = 1.25;
        startup.launch_settings.display.hdr = newengine_core::StartupHdrMode::Enabled;
        startup.launch_settings_explicit = true;

        let config = platform_config_from_startup_defaults(&startup);

        assert_eq!((config.width, config.height), (2560, 1440));
        assert_eq!(config.display.monitor_index, 2);
        assert_eq!(
            config.display.window_mode,
            PlatformWindowModeV1::ExclusiveFullscreen
        );
        assert!(!config.display.vsync);
        assert_eq!(config.display.refresh_rate_millihz, 165_000);
        assert_eq!(config.display.render_scale, 1.25);
        assert_eq!(config.display.hdr, PlatformHdrModeV1::Enabled);
    }

    #[test]
    fn confirmed_core_settings_have_last_priority_over_platform_plugin_config() {
        let mut startup = StartupConfig {
            window_size: (1920, 1080),
            ..StartupConfig::default()
        };
        startup.launch_settings.display.window_mode = newengine_core::StartupWindowMode::Borderless;
        startup.launch_settings.display.vsync = false;
        startup.launch_settings.display.render_scale = 0.8;
        startup.launch_settings_explicit = true;

        let mut plugin_config = PlatformAppConfigV1 {
            width: 800,
            height: 600,
            ..PlatformAppConfigV1::default()
        };
        plugin_config.display.window_mode = PlatformWindowModeV1::Windowed;
        plugin_config.display.vsync = true;
        plugin_config.display.render_scale = 2.0;

        let effective = apply_confirmed_core_launch_settings(plugin_config, &startup);

        assert_eq!((effective.width, effective.height), (1920, 1080));
        assert_eq!(
            effective.display.window_mode,
            PlatformWindowModeV1::Borderless
        );
        assert!(!effective.display.vsync);
        assert_eq!(effective.display.render_scale, 0.8);
    }
}
