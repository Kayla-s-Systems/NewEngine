#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{HostApiV1, PluginInfo, PluginModule};

use crate::service::AudioNullService;

pub(crate) struct AudioNullModule {
    host: Option<HostApiV1>,
}

impl AudioNullModule {
    pub(crate) fn new() -> Self {
        Self { host: None }
    }

    fn log_info(&self, msg: &str) {
        if let Some(host) = &self.host {
            (host.log_info)(RString::from(msg));
        }
    }
}

impl PluginModule for AudioNullModule {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("newengine.modules.audio.null"),
            name: RString::from("NewEngine Audio (Null)"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        self.host = Some(host.clone());

        let svc = AudioNullService::new(host);
        let svc_dyn = newengine_plugin_api::ServiceV1Dyn::from_value(
            svc,
            abi_stable::std_types::RBox::new(()),
        );

        let res = (host.register_service_v1)(svc_dyn);
        if res.is_ok() {
            self.log_info("audio-null: registered service newengine.audio.v1");
        }
        res
    }

    fn start(&mut self) -> RResult<(), RString> {
        self.log_info("audio-null: start");
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
        self.host = None;
    }
}
