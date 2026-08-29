#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{Blob, CapabilityId, HostApiV1, MethodName};
use std::cell::Cell;
use std::sync::OnceLock;

const ENGINE_LOG_SERVICE_ID: &str = "engine.logging";
const METHOD_WRITE_EVENT_JSON: &str = "write_event_json";

thread_local! {
    static IN_ULOG_EVENT_EMIT: Cell<bool> = const { Cell::new(false) };
}

fn host_run_id() -> &'static str {
    static RUN_ID: OnceLock<String> = OnceLock::new();
    RUN_ID
        .get_or_init(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            format!("HOST-RUN-{ms}")
        })
        .as_str()
}

#[inline]
pub fn install_structured_ulog_sink_once() {
    let _ = newengine_ulog_api::install_event_sink_once(emit_structured_ulog_event);
}

fn emit_structured_ulog_event(event: newengine_ulog_api::UlogEvent) {
    emit_ulog_event(
        &crate::host_api::default_host_api(),
        event.event_id.as_str(),
        event.level.as_str(),
        event.message.as_str(),
        serde_json::json!({
            "target": event.target,
            "module_path": event.module_path,
            "file": event.file,
            "line": event.line,
            "fields": event.fields,
        }),
    );
}

pub fn emit_ulog_event(
    host: &HostApiV1,
    event_id: &str,
    level: &str,
    message: &str,
    fields: serde_json::Value,
) {
    IN_ULOG_EVENT_EMIT.with(|guard| {
        if guard.get() {
            return;
        }
        guard.set(true);

        struct Restore<'a>(&'a Cell<bool>);
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _restore = Restore(guard);

        if !crate::host_context::has_service(ENGINE_LOG_SERVICE_ID) {
            return;
        }

        let payload = serde_json::json!({
            "run_id": host_run_id(),
            "level": level,
            "target": "newengine-plugin-host",
            "message": message,
            "event_id": event_id,
            "fields": fields
        });

        let bytes = match serde_json::to_vec(&payload) {
            Ok(v) => v,
            Err(_) => return,
        };

        let _ = (host.call_service_v1)(
            CapabilityId::from(ENGINE_LOG_SERVICE_ID),
            MethodName::from(METHOD_WRITE_EVENT_JSON),
            Blob::from(bytes),
        );
    });
}
