use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, EventSinkV1Dyn};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use crate::host_context::unregister_by_owner;
use super::state::{ctx, current_plugin_id, with_current_plugin_id, EventSinkEntry};

pub fn subscribe_event_sink(sink: EventSinkV1Dyn<'static>) -> Result<(), String> {
    let c = ctx();
    let mut g = match c.event_sinks.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    g.push(EventSinkEntry {
        owner_plugin_id: current_plugin_id(),
        sink: Arc::new(Mutex::new(sink)),
    });

    Ok(())
}

pub fn publish_event(topic: &str, payload: &[u8]) -> Result<(), String> {
    let c = ctx();

    let sinks: Vec<EventSinkEntry> = {
        let g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.clone()
    };

    // Avoid per-sink payload construction by cloning a single Vec.
    let payload_vec: Vec<u8> = payload.to_vec();

    let mut bad_owners: Vec<String> = Vec::new();

    for s in sinks {
        let owner = s.owner_plugin_id.clone();

        let mut guard = match s.sink.lock() {
            Ok(v) => v,
            Err(_) => {
                if let Some(pid) = owner {
                    newengine_ulog_api::ulog::error!(
                        "events: sink mutex poisoned; owner='{}' topic='{}' (auto-unregister)",
                        pid,
                        topic
                    );
                    bad_owners.push(pid);
                } else {
                    newengine_ulog_api::ulog::error!("events: sink mutex poisoned; owner=<host> topic='{}'", topic);
                }
                continue;
            }
        };

        let call = || {
            // Blob is consumed by on_event(); clone bytes per sink.
            let _ = guard.on_event(RString::from(topic), Blob::from(payload_vec.clone()));
        };

        let r = match owner.as_deref() {
            Some(pid) => catch_unwind(AssertUnwindSafe(|| with_current_plugin_id(pid, call))),
            None => catch_unwind(AssertUnwindSafe(call)),
        };

        if r.is_err() {
            if let Some(pid) = owner {
                newengine_ulog_api::ulog::error!(
                    "events: sink panicked; owner='{}' topic='{}' (auto-unregister)",
                    pid,
                    topic
                );
                bad_owners.push(pid);
            } else {
                newengine_ulog_api::ulog::error!("events: sink panicked; owner=<host> topic='{}'", topic);
            }
        }
    }

    if !bad_owners.is_empty() {
        bad_owners.sort();
        bad_owners.dedup();
        for pid in bad_owners {
            unregister_by_owner(&pid);
        }
    }

    Ok(())
}

/// Emits an event originating from a plugin (ABI surface: `HostApiV1.emit_event_v1`).
#[inline]
pub fn emit_plugin_event(topic: RString, payload: Blob) -> Result<(), String> {
    publish_event(topic.as_str(), payload.as_slice())
}
