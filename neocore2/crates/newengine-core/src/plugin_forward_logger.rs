#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{Blob, CapabilityId, HostApiV1, MethodName};
use serde::Serialize;
use std::cell::Cell;
use std::sync::OnceLock;

/// Engine-facing logging gateway used by the host logger forwarder.
///
/// The concrete logging sink remains plugin-owned; the host resolves this
/// gateway from provider metadata instead of calling the provider id directly.
pub const ENGINE_LOG_SERVICE_ID: &str = "engine.log";

/// First-party provider service id exposed by the logging plugin.
pub const LOGGING_SINK_SERVICE_ID: &str = "newengine.logging.sink.v1";

const METHOD_WRITE_JSON: &str = "write_json";
const METHOD_FLUSH: &str = "flush";

#[derive(Debug, Clone, Serialize)]
struct LogRecordWire<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_tag: Option<&'a str>,

    level: &'a str,
    target: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    message: String,
}

struct ForwardToPluginLogger {
    host: HostApiV1,
    sink_id: CapabilityId,
}

thread_local! {
    static IN_FORWARD_LOG: Cell<bool> = const { Cell::new(false) };
}

#[inline]
fn with_reentrancy_guard(f: impl FnOnce()) {
    IN_FORWARD_LOG.with(|c| {
        if c.get() {
            return;
        }
        c.set(true);

        struct Restore<'a>(&'a Cell<bool>);
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }

        let _restore = Restore(c);
        f();
    })
}

impl ForwardToPluginLogger {
    #[inline]
    fn send_json(&self, method: &str, json: Vec<u8>) {
        let _ = (self.host.call_service_v1)(
            self.sink_id.clone(),
            MethodName::from(method),
            Blob::from(json),
        );
    }
}

impl log::Log for ForwardToPluginLogger {
    #[inline]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // The plugin owns filtering.
        // Keep this permissive to avoid dropping records before they reach the plugin.
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        with_reentrancy_guard(|| {
            let wire = LogRecordWire {
                run_id: crate::run_id::run_id(),
                run_tag: crate::run_id::run_tag(),

                level: record.level().as_str(),
                target: record.target(),
                module_path: record.module_path(),
                file: record.file(),
                line: record.line(),
                message: record.args().to_string(),
            };

            let json = match serde_json::to_vec(&wire) {
                Ok(v) => v,
                Err(_) => return,
            };

            self.send_json(METHOD_WRITE_JSON, json);
        });
    }

    fn flush(&self) {
        with_reentrancy_guard(|| {
            self.send_json(METHOD_FLUSH, Vec::new());
        });
    }
}

// --- Installation ---------------------------------------------------------

static INSTALLED: OnceLock<()> = OnceLock::new();

/// Best-effort, idempotent installation of the host-side logger forwarder.
///
/// This MUST run in the host (exe) because a logger installed inside a plugin DLL
/// does not affect the host's `log` global.
///
/// Contract:
/// - If the logging plugin is not loaded (service missing), this function is a no-op.
/// - If a logger was already installed, this function is a no-op.
pub fn install_forward_logger_once(host: HostApiV1) {
    if INSTALLED.get().is_some() {
        return;
    }

    if !newengine_plugin_host::has_service(ENGINE_LOG_SERVICE_ID) {
        return;
    }

    // Install process-wide logger that forwards to the plugin service.
    // If some other logger was installed, respect it and do nothing.
    let logger = ForwardToPluginLogger {
        host,
        sink_id: CapabilityId::from(ENGINE_LOG_SERVICE_ID),
    };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
        let _ = INSTALLED.set(());
    }
}