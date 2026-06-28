#![forbid(unsafe_op_in_unsafe_fn)]

//! North Star ULOG facade.
//!
//! Structured ULOG events are the canonical logging surface. Legacy `ulog::info!`
//! style macros remain as a migration shim for old formatted callsites, but new
//! code should prefer `ulog::event!` or the level-specific `*_event!` macros.

use std::sync::OnceLock;

#[doc(hidden)]
pub mod __private {
    pub use log;
    pub use serde_json;
}

/// Canonical structured ULOG event payload emitted by engine-side callsites.
#[derive(Debug, Clone)]
pub struct UlogEvent {
    pub event_id: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub fields: serde_json::Value,
}

pub type UlogEventSink = fn(UlogEvent);

static EVENT_SINK: OnceLock<UlogEventSink> = OnceLock::new();

/// Installs the process-local structured ULOG event sink.
///
/// The host owns the concrete transport. This API intentionally accepts only a
/// function pointer so the facade stays dependency-free and can be called from
/// engine/runtime crates without knowing about plugin-host internals.
pub fn install_event_sink_once(sink: UlogEventSink) -> bool {
    EVENT_SINK.set(sink).is_ok()
}

#[inline]
pub fn structured_event_sink_installed() -> bool {
    EVENT_SINK.get().is_some()
}

#[inline]
pub fn emit_event(
    event_id: impl Into<String>,
    level: impl Into<String>,
    target: &'static str,
    message: impl Into<String>,
    module_path: Option<&'static str>,
    file: Option<&'static str>,
    line: Option<u32>,
    fields: serde_json::Value,
) {
    let level = normalize_level(level.into());
    let message = message.into();
    let event = UlogEvent {
        event_id: event_id.into(),
        level: level.clone(),
        target: target.to_owned(),
        message: message.clone(),
        module_path: module_path.map(str::to_owned),
        file: file.map(str::to_owned),
        line,
        fields,
    };

    if let Some(sink) = EVENT_SINK.get().copied() {
        sink(event);
        return;
    }

    // Transitional fallback for very early startup before plugin-host has
    // installed the structured sink. This keeps old visibility without making
    // `log::*` the canonical path.
    match level.as_str() {
        "ERROR" => log::error!(target: target, "{}", message),
        "WARN" => log::warn!(target: target, "{}", message),
        "DEBUG" => log::debug!(target: target, "{}", message),
        "TRACE" => log::trace!(target: target, "{}", message),
        _ => log::info!(target: target, "{}", message),
    }
}

#[inline]
fn normalize_level(level: String) -> String {
    match level.as_str() {
        "ERROR" | "Error" | "error" => "ERROR".to_owned(),
        "WARN" | "Warn" | "warn" | "WARNING" | "Warning" | "warning" => "WARN".to_owned(),
        "DEBUG" | "Debug" | "debug" => "DEBUG".to_owned(),
        "TRACE" | "Trace" | "trace" => "TRACE".to_owned(),
        _ => "INFO".to_owned(),
    }
}

/// Re-exported macro namespace used by engine callsites.
pub mod ulog {
    pub use crate::{
        debug, debug_enabled, debug_event, enabled, error, error_event, event, info, info_event,
        structured_event_sink_installed, trace, trace_enabled, trace_event, warn, warn_event,
    };
}

#[inline]
pub fn enabled(level: log::Level) -> bool {
    log::log_enabled!(level)
}

#[inline]
pub fn debug_enabled() -> bool {
    enabled(log::Level::Debug)
}

#[inline]
pub fn trace_enabled() -> bool {
    enabled(log::Level::Trace)
}

#[macro_export]
macro_rules! event {
    ($event_id:expr, $level:ident, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::emit_event(
            $event_id,
            stringify!($level),
            module_path!(),
            $message,
            Some(module_path!()),
            Some(file!()),
            Some(line!()),
            $crate::__private::serde_json::json!({ $($fields)* }),
        )
    };
    ($event_id:expr, $level:ident, $message:expr $(,)?) => {
        $crate::emit_event(
            $event_id,
            stringify!($level),
            module_path!(),
            $message,
            Some(module_path!()),
            Some(file!()),
            Some(line!()),
            $crate::__private::serde_json::json!({}),
        )
    };
}

#[macro_export]
macro_rules! info_event {
    ($event_id:expr, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::event!($event_id, INFO, $message, { $($fields)* })
    };
    ($event_id:expr, $message:expr $(,)?) => {
        $crate::event!($event_id, INFO, $message)
    };
}

#[macro_export]
macro_rules! warn_event {
    ($event_id:expr, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::event!($event_id, WARN, $message, { $($fields)* })
    };
    ($event_id:expr, $message:expr $(,)?) => {
        $crate::event!($event_id, WARN, $message)
    };
}

#[macro_export]
macro_rules! error_event {
    ($event_id:expr, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::event!($event_id, ERROR, $message, { $($fields)* })
    };
    ($event_id:expr, $message:expr $(,)?) => {
        $crate::event!($event_id, ERROR, $message)
    };
}

#[macro_export]
macro_rules! debug_event {
    ($event_id:expr, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::event!($event_id, DEBUG, $message, { $($fields)* })
    };
    ($event_id:expr, $message:expr $(,)?) => {
        $crate::event!($event_id, DEBUG, $message)
    };
}

#[macro_export]
macro_rules! trace_event {
    ($event_id:expr, $message:expr, { $($fields:tt)* } $(,)?) => {
        $crate::event!($event_id, TRACE, $message, { $($fields)* })
    };
    ($event_id:expr, $message:expr $(,)?) => {
        $crate::event!($event_id, TRACE, $message)
    };
}

// Transitional formatted macros. These remain for incremental migration only.
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        $crate::__private::log::info!($($arg)+)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        $crate::__private::log::warn!($($arg)+)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        $crate::__private::log::error!($($arg)+)
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        $crate::__private::log::debug!($($arg)+)
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {
        $crate::__private::log::trace!($($arg)+)
    };
}
