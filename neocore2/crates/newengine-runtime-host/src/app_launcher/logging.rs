use std::{
    fmt,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use newengine_core::StartupConfig;

use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};
use crate::asset_bootstrap::shard_log_path_by_run_id;

static APP_LAUNCH_EARLY_SEQ: AtomicU64 = AtomicU64::new(1);
const CHRONICLE_PLUGIN_ID: &str = "engine.logging.chronicle";
const PLATFORM_EARLY_LOG_ENV: &str = "NEWENGINE_PLATFORM_EARLY_LOG";
const WINT_EARLY_LOG_ENV: &str = "NEWENGINE_WINIT_EARLY_LOG";

fn logging_source_enabled(
    logging: &serde_json::Map<String, serde_json::Value>,
    source: &str,
) -> bool {
    let explicit = logging
        .get(source)
        .or_else(|| logging.get("sources").and_then(|v| v.get(source)))
        .or_else(|| logging.get("outputs").and_then(|v| v.get(source)));
    match explicit {
        Some(serde_json::Value::Bool(enabled)) => *enabled,
        Some(serde_json::Value::Object(object)) => object
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        Some(serde_json::Value::String(value)) => {
            let value = value.trim();
            !value.is_empty()
                && !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "0" | "false" | "off" | "disabled"
                )
        }
        Some(_) => true,
        None => true,
    }
}

fn configured_logging_path(
    logging: &serde_json::Map<String, serde_json::Value>,
    source: &str,
) -> Option<String> {
    let legacy = match source {
        "file" => logging.get("file").or_else(|| logging.get("file_path")),
        "ulog" => logging.get("ulog_path").or_else(|| logging.get("ulog")),
        _ => None,
    };
    legacy
        .and_then(logging_path_value)
        .or_else(|| {
            logging
                .get("sources")
                .and_then(|v| v.get(source))
                .and_then(logging_path_value)
        })
        .or_else(|| {
            logging
                .get("outputs")
                .and_then(|v| v.get(source))
                .and_then(logging_path_value)
        })
        .map(str::to_owned)
}

fn logging_path_value(value: &serde_json::Value) -> Option<&str> {
    match value {
        serde_json::Value::String(path) => {
            let path = path.trim();
            (!path.is_empty()).then_some(path)
        }
        serde_json::Value::Object(object) => object
            .get("path")
            .or_else(|| object.get("file"))
            .or_else(|| object.get("file_path"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty()),
        _ => None,
    }
}

fn set_logging_source_path(
    logging: &mut serde_json::Map<String, serde_json::Value>,
    source: &str,
    path: &str,
) {
    let sources = logging
        .entry("sources".to_owned())
        .or_insert_with(|| serde_json::json!({}));
    if !sources.is_object() {
        *sources = serde_json::json!({});
    }
    let sources = sources
        .as_object_mut()
        .expect("sources normalized to object");
    let value = sources
        .entry(source.to_owned())
        .or_insert_with(|| serde_json::json!({}));
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    value
        .as_object_mut()
        .expect("logging source normalized to object")
        .insert(
            "path".to_owned(),
            serde_json::Value::String(path.to_owned()),
        );
}

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    pub(super) fn configure_sharded_log_files(&self, startup: &mut StartupConfig, run_id: &str) {
        let Some(logging) = startup
            .plugins
            .get_mut(CHRONICLE_PLUGIN_ID)
            .and_then(serde_json::Value::as_object_mut)
        else {
            return;
        };

        if std::env::var_os("NEWENGINE_LOG_FILE").is_none() {
            if let Some(path) = configured_logging_path(logging, "file") {
                if let Some(sharded) = shard_log_path_by_run_id(&path, run_id) {
                    set_logging_source_path(logging, "file", &sharded);
                    std::env::set_var("NEWENGINE_LOG_FILE", &sharded);
                    self.early_log(format_args!(
                        "logging.file.sharded path={} run_id={}",
                        sharded, run_id
                    ));
                }
            }
        }

        if std::env::var_os("NORTHSTAR_ULOG").is_none()
            && std::env::var_os("NEWENGINE_ULOG").is_none()
            && logging_source_enabled(logging, "ulog")
        {
            let path = configured_logging_path(logging, "ulog")
                .unwrap_or_else(|| "logs/current.ulog.ndjson".to_owned());
            if let Some(sharded) = shard_log_path_by_run_id(&path, run_id) {
                set_logging_source_path(logging, "ulog", &sharded);
                std::env::set_var("NEWENGINE_ULOG", &sharded);
                self.early_log(format_args!(
                    "logging.ulog.sharded path={} run_id={}",
                    sharded, run_id
                ));
            }
        }
    }

    pub(super) fn early_log(&self, args: fmt::Arguments<'_>) {
        let seq = APP_LAUNCH_EARLY_SEQ.fetch_add(1, Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let payload = serde_json::json!({
            "schema": "northstar.ulog.event.v1",
            "timestamp_utc": format!("{}.{}Z", now_ms / 1000, now_ms % 1000),
            "level": "DEBUG",
            "event_id": "engine.runtime_host.early",
            "message": args.to_string(),
            "source": { "kind": "engine", "name": "newengine-runtime-host" },
            "context": { "run_id": null, "session_id": null },
            "location": {
                "module": "newengine_runtime_host::app_launcher",
                "file": null,
                "line": null
            },
            "fields": {
                "app_name": self.spec.app_name,
                "early_source": self.spec.early_log_file_name,
                "sequence": seq
            }
        });
        let Ok(line) = serde_json::to_string(&payload) else {
            return;
        };
        for path in self.early_log_path_candidates() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            else {
                continue;
            };
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
            return;
        }
    }

    pub(super) fn prepare_early_log_session(&self) {
        if std::env::var_os(PLATFORM_EARLY_LOG_ENV).is_some()
            || std::env::var_os(WINT_EARLY_LOG_ENV).is_some()
        {
            return;
        }
        let canonical = canonical_early_ulog_path();
        let Ok(metadata) = std::fs::metadata(&canonical) else {
            return;
        };
        if metadata.len() == 0 {
            let _ = std::fs::remove_file(canonical);
            return;
        }
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or(0);
        let orphan = canonical.with_file_name(format!(
            "current.ulog.orphan.{}.{}.ndjson",
            std::process::id(),
            unix_ms
        ));
        let _ = std::fs::rename(canonical, orphan);
    }

    pub(super) fn bind_early_log_to_run(&self, run_id: &str) {
        let explicit_platform =
            std::env::var_os(PLATFORM_EARLY_LOG_ENV).filter(|value| !value.is_empty());
        let explicit_winit = std::env::var_os(WINT_EARLY_LOG_ENV).filter(|value| !value.is_empty());
        if explicit_platform.is_some() || explicit_winit.is_some() {
            if explicit_winit.is_none() {
                if let Some(path) = explicit_platform {
                    std::env::set_var(WINT_EARLY_LOG_ENV, path);
                }
            }
            return;
        }
        let canonical = canonical_early_ulog_path();
        let sharded = early_ulog_path_by_run_id(&canonical, run_id);
        if let Some(parent) = sharded.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if canonical.exists() {
            let _ = std::fs::rename(&canonical, &sharded);
        }
        std::env::set_var(PLATFORM_EARLY_LOG_ENV, &sharded);
        std::env::set_var(WINT_EARLY_LOG_ENV, &sharded);
    }

    fn early_log_path_candidates(&self) -> Vec<PathBuf> {
        if let Some(path) =
            std::env::var_os(PLATFORM_EARLY_LOG_ENV).filter(|value| !value.is_empty())
        {
            return vec![PathBuf::from(path)];
        }
        vec![canonical_early_ulog_path()]
    }
}

fn canonical_early_ulog_path() -> PathBuf {
    cache_root_from_env_or_neocore2()
        .join("logs")
        .join("current.ulog.ndjson")
}

fn early_ulog_path_by_run_id(canonical: &Path, run_id: &str) -> PathBuf {
    canonical.with_file_name(format!("current.ulog.{run_id}.bootstrap.ndjson"))
}

fn cache_root_from_env_or_neocore2() -> PathBuf {
    if std::env::var_os(newengine_core::CACHE_FILES_READY_ENV).is_some() {
        if let Some(path) = std::env::var_os(newengine_core::CACHE_FILES_ENV)
            .or_else(|| std::env::var_os(newengine_core::CACHE_FILES_ALIAS_ENV))
            .filter(|v| !v.as_os_str().is_empty())
        {
            return PathBuf::from(path);
        }
    }
    crate::path_resolver::find_neocore2_root().join("cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_logging_paths_are_discovered_and_mutated() {
        let mut logging = serde_json::json!({
            "sources": {
                "file": { "enabled": true, "path": "logs/game.log" },
                "ulog": { "enabled": true, "path": "logs/current.ulog.ndjson" }
            }
        })
        .as_object()
        .expect("object")
        .clone();
        assert_eq!(
            configured_logging_path(&logging, "file").as_deref(),
            Some("logs/game.log")
        );
        assert_eq!(
            configured_logging_path(&logging, "ulog").as_deref(),
            Some("logs/current.ulog.ndjson")
        );
        set_logging_source_path(&mut logging, "ulog", "logs/run.ulog.ndjsond");
        assert_eq!(
            configured_logging_path(&logging, "ulog").as_deref(),
            Some("logs/run.ulog.ndjsond")
        );
    }

    #[test]
    fn explicit_ulog_disable_is_preserved() {
        let logging = serde_json::json!({
            "sources": { "ulog": { "enabled": false, "path": "logs/current.ulog.ndjson" } }
        })
        .as_object()
        .expect("object")
        .clone();
        assert!(!logging_source_enabled(&logging, "ulog"));
    }

    #[test]
    fn legacy_paths_remain_supported() {
        let logging = serde_json::json!({
            "file_path": "logs/legacy.log",
            "ulog_path": "logs/legacy.ulog.ndjsond"
        })
        .as_object()
        .expect("object")
        .clone();
        assert_eq!(
            configured_logging_path(&logging, "file").as_deref(),
            Some("logs/legacy.log")
        );
        assert_eq!(
            configured_logging_path(&logging, "ulog").as_deref(),
            Some("logs/legacy.ulog.ndjsond")
        );
    }

    #[test]
    fn bootstrap_early_log_is_sharded_by_run_id() {
        let canonical = PathBuf::from("cache/logs/current.ulog.ndjson");
        assert_eq!(
            early_ulog_path_by_run_id(&canonical, "RUN-123"),
            PathBuf::from("cache/logs/current.ulog.RUN-123.bootstrap.ndjson")
        );
    }
}
