#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, CapabilityDesc, CapabilityId, HostApiV1, MethodName, PluginDescriptor, PluginInfo,
    PluginKind, PluginModule, PluginModuleV2, ServiceV1, ServiceV1Dyn,
};

use crate::logger::{build_env_logger, ConsoleLoggerConfig};

use log::Log;
use serde::Deserialize;
use std::sync::OnceLock;

const STARTUP_CONFIG_SERVICE_ID: &str = "newengine.startup.config.v1";
const STARTUP_CONFIG_METHOD_GET_JSON: &str = "get_json";

const LOGGING_SINK_SERVICE_ID: &str = "newengine.logging.sink.v1";
const METHOD_WRITE_JSON: &str = "write_json";
const METHOD_FLUSH: &str = "flush";

fn resolve_logging_config(host: &HostApiV1) -> Result<ConsoleLoggerConfig, String> {
    let cap = CapabilityId::from(STARTUP_CONFIG_SERVICE_ID);
    let method = MethodName::from(STARTUP_CONFIG_METHOD_GET_JSON);
    let payload = Blob::from(Vec::from("logging".as_bytes()));

    let out = (host.call_service_v1)(cap, method, payload)
        .into_result()
        .map_err(|e| e.to_string())?;

    let json = String::from_utf8(out.as_slice().to_vec())
        .map_err(|e| format!("startup config service returned non-utf8: {e}"))?;

    ConsoleLoggerConfig::from_host_json(&json)
}

#[derive(Debug, Clone, Deserialize)]
struct LogRecordWire {
    level: String,
    target: String,
    #[serde(default)]
    module_path: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    message: String,
}

struct LoggingRuntime {
    logger: env_logger::Logger,
}

static RUNTIME: OnceLock<LoggingRuntime> = OnceLock::new();

#[derive(Default, StableAbi)]
#[repr(C)]
struct LoggingSinkService;

fn parse_level(s: &str) -> Option<log::Level> {
    match s {
        "ERROR" | "Error" | "error" => Some(log::Level::Error),
        "WARN" | "Warn" | "warn" => Some(log::Level::Warn),
        "INFO" | "Info" | "info" => Some(log::Level::Info),
        "DEBUG" | "Debug" | "debug" => Some(log::Level::Debug),
        "TRACE" | "Trace" | "trace" => Some(log::Level::Trace),
        _ => None,
    }
}

impl ServiceV1 for LoggingSinkService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(LOGGING_SINK_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        // Minimal stable contract description for tooling.
        // Methods:
        // - write_json: accepts JSON encoded LogRecordWire.
        // - flush: flushes underlying sinks.
        RString::from(r#"{"id":"newengine.logging.sink.v1","methods":["write_json","flush"]}"#)
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let Some(rt) = RUNTIME.get() else {
            return RResult::RErr(RString::from("logging runtime not initialized"));
        };

        match method.as_str() {
            METHOD_WRITE_JSON => {
                let wire: LogRecordWire = match serde_json::from_slice(payload.as_slice()) {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(format!("bad log json: {e}"))),
                };

                let Some(level) = parse_level(&wire.level) else {
                    return RResult::RErr(RString::from("bad log level"));
                };

                // Build Record referencing data owned by `wire` (kept alive until after log()).
                let mut rb = log::Record::builder();
                rb.level(level).target(&wire.target);

                if let Some(mp) = wire.module_path.as_deref() {
                    rb.module_path(Some(mp));
                }
                if let Some(f) = wire.file.as_deref() {
                    rb.file(Some(f));
                }
                if let Some(l) = wire.line {
                    rb.line(Some(l));
                }

                // Keep `Arguments` alive long enough (E0716 fix).
                let args = format_args!("{}", wire.message);
                let rec = rb.args(args).build();

                rt.logger.log(&rec);
                RResult::ROk(Blob::new())
            }
            METHOD_FLUSH => {
                rt.logger.flush();
                RResult::ROk(Blob::new())
            }
            _ => RResult::RErr(RString::from(format!("unknown method: {method}"))),
        }
    }
}

#[derive(Default, StableAbi)]
#[repr(C)]
pub struct LoggingPlugin {
    initialized: bool,
}

impl LoggingPlugin {
    fn init_impl(&mut self, host: HostApiV1) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        let cfg = resolve_logging_config(&host).unwrap_or_else(|_| ConsoleLoggerConfig::from_env());

        // Build plugin-owned logger instance (NOT global).
        let logger = build_env_logger(&cfg)?;
        let _ = RUNTIME.set(LoggingRuntime { logger });

        // Register sink service. The host installs the global backend and forwards into it.
        let svc: ServiceV1Dyn<'static> = ServiceV1Dyn::from_value(
            LoggingSinkService::default(),
            abi_stable::sabi_trait::TD_Opaque,
        );

        (host.register_service_v1)(svc)
            .into_result()
            .map_err(|e| e.to_string())?;

        self.initialized = true;
        Ok(())
    }

    fn descriptor_impl(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: RString::from("newengine.logging"),
            name: RString::from("NewEngine Logging"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
            kind: PluginKind::Runtime,
            capabilities: RVec::<CapabilityDesc>::new(),
        }
    }

    fn info_impl(&self) -> PluginInfo {
        let d = self.descriptor_impl();
        PluginInfo {
            id: d.id,
            name: d.name,
            version: d.version,
        }
    }
}

impl PluginModuleV2 for LoggingPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor_impl()
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        match self.init_impl(host) {
            Ok(()) => RResult::ROk(()),
            Err(e) => RResult::RErr(RString::from(e)),
        }
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {
        // No-op: host-installed global logger is process-wide; plugin provides the sink service.
    }
}

impl PluginModule for LoggingPlugin {
    fn info(&self) -> PluginInfo {
        self.info_impl()
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        <Self as PluginModuleV2>::init(self, host)
    }

    fn start(&mut self) -> RResult<(), RString> {
        <Self as PluginModuleV2>::start(self)
    }

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::fixed_update(self, dt)
    }

    fn update(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::update(self, dt)
    }

    fn render(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::render(self, dt)
    }

    fn shutdown(&mut self) {
        <Self as PluginModuleV2>::shutdown(self)
    }
}