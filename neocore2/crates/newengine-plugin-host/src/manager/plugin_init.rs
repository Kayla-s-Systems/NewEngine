#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::time::Instant;

use abi_stable::std_types::RVec;
use newengine_plugin_api::{ConfigDiagLevelV1, ConfigPatchV1, HostApiV1};

use crate::host_context::with_current_plugin_id;

use super::adapter::ModuleAdapterAny;
use super::config_diff::json_diff_keys_shallow_or_paths;
use super::config_patch::config_patch_from_json_merge_patch;
use super::load_profile::InitTimings;
use super::types::PluginLoadError;

const CONFIG_PREVIEW_MAX_BYTES: usize = 200;
const CONFIG_PREVIEW_SUFFIX: &str = "...";

pub(super) fn init_with_overrides(
    module_any: &mut ModuleAdapterAny,
    id_str: &str,
    host: HostApiV1,
    overrides_non_empty: bool,
    overrides: &serde_json::Value,
) -> Result<InitTimings, String> {
    let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_plugin_id(id_str, || {
            let mut timings = InitTimings::default();

            let started = Instant::now();
            let defaults = module_any
                .module
                .config_defaults()
                .into_result()
                .map_err(|error| error.to_string())?;
            timings.config_defaults_ms = started.elapsed().as_millis();

            newengine_ulog_api::ulog::debug!(
                "plugins: config defaults id='{}' content_type='{}' len={} fmt_v={} ",
                id_str,
                defaults.content_type,
                defaults.bytes.len(),
                defaults.format_version
            );

            let mut patches = RVec::<ConfigPatchV1>::new();
            if overrides_non_empty {
                patches.push(config_patch_from_json_merge_patch(
                    id_str,
                    "config+env",
                    0,
                    overrides,
                ));
            }

            let started = Instant::now();
            let applied = module_any
                .module
                .config_apply_patches(&defaults, patches)
                .into_result()
                .map_err(|error| error.to_string())?;
            timings.config_apply_ms = started.elapsed().as_millis();

            // Preview construction and JSON diffing are diagnostics-only. Keeping this
            // behind the runtime log gate removes two parses and several allocations
            // from every release plugin initialization.
            if newengine_ulog_api::ulog::debug_enabled() {
                let preview = config_preview(applied.effective.bytes.as_slice());
                let changed_keys = json_diff_keys_shallow_or_paths(
                    defaults.content_type.as_str(),
                    defaults.bytes.as_slice(),
                    applied.effective.bytes.as_slice(),
                );

                if changed_keys.is_empty() {
                    newengine_ulog_api::ulog::debug!(
                        "plugins: config effective id='{}' content_type='{}' len={} changed={} preview='{}'",
                        id_str,
                        applied.effective.content_type,
                        applied.effective.bytes.len(),
                        applied.changed,
                        preview
                    );
                } else {
                    newengine_ulog_api::ulog::debug!(
                        "plugins: config effective id='{}' content_type='{}' len={} changed={} changed_keys=[{}] preview='{}'",
                        id_str,
                        applied.effective.content_type,
                        applied.effective.bytes.len(),
                        applied.changed,
                        changed_keys.join(", "),
                        preview
                    );
                }
            }

            for diagnostic in applied.diags.iter() {
                match diagnostic.level {
                    ConfigDiagLevelV1::Info => newengine_ulog_api::ulog::info!(
                        "plugins: config info id='{}' {} {}",
                        id_str,
                        diagnostic.code,
                        diagnostic.message
                    ),
                    ConfigDiagLevelV1::Warn => newengine_ulog_api::ulog::warn!(
                        "plugins: config warn id='{}' {} {}",
                        id_str,
                        diagnostic.code,
                        diagnostic.message
                    ),
                    ConfigDiagLevelV1::Error => newengine_ulog_api::ulog::error!(
                        "plugins: config error id='{}' {} {}",
                        id_str,
                        diagnostic.code,
                        diagnostic.message
                    ),
                }
            }

            let started = Instant::now();
            module_any
                .module
                .init(host.clone(), applied.effective)
                .into_result()
                .map_err(|error| error.to_string())?;
            timings.init_ms = started.elapsed().as_millis();
            Ok(timings)
        })
    }));

    match init_res {
        Ok(Ok(timings)) => Ok(timings),
        Ok(Err(error)) => {
            shutdown_after_failed_init(id_str, module_any);
            Err(error)
        }
        Err(_) => {
            shutdown_after_failed_init(id_str, module_any);
            Err("init panicked".to_owned())
        }
    }
}

fn config_preview(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    if decoded.len() <= CONFIG_PREVIEW_MAX_BYTES {
        return decoded.into_owned();
    }

    let mut end = CONFIG_PREVIEW_MAX_BYTES;
    while end > 0 && !decoded.is_char_boundary(end) {
        end -= 1;
    }

    let mut preview = String::with_capacity(end + CONFIG_PREVIEW_SUFFIX.len());
    preview.push_str(&decoded[..end]);
    preview.push_str(CONFIG_PREVIEW_SUFFIX);
    preview
}

pub(super) fn shutdown_after_failed_init(id_str: &str, module_any: &mut ModuleAdapterAny) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_plugin_id(id_str, || module_any.module.shutdown())
    }));
}

pub(super) fn shutdown_adapter_any(mut module_any: ModuleAdapterAny) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        module_any.shutdown();
    }));
}

pub(super) fn require_non_empty(
    field: &str,
    value: &str,
    path: &Path,
) -> Result<(), PluginLoadError> {
    if value.trim().is_empty() {
        return Err(PluginLoadError {
            path: path.to_path_buf(),
            message: format!("{field} is empty"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_preview_preserves_utf8_boundary() {
        let source = "Ж".repeat(CONFIG_PREVIEW_MAX_BYTES);
        let preview = config_preview(source.as_bytes());

        assert!(preview.ends_with(CONFIG_PREVIEW_SUFFIX));
        assert!(preview.is_char_boundary(preview.len()));
        assert!(preview.len() <= CONFIG_PREVIEW_MAX_BYTES + CONFIG_PREVIEW_SUFFIX.len());
    }

    #[test]
    fn short_config_preview_is_unchanged() {
        assert_eq!(
            config_preview(br#"{"enabled":true}"#),
            r#"{"enabled":true}"#
        );
    }
}
