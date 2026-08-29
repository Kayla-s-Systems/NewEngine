use newengine_core::EngineRunState;

use super::super::HostPlatformRuntime;

impl HostPlatformRuntime {
    pub(crate) fn shutdown_engine_once(&mut self, origin: &'static str) {
        if self.shutting_down
            || matches!(
                self.engine.run_state(),
                EngineRunState::Stopped | EngineRunState::Faulted
            )
        {
            return;
        }
        self.shutting_down = true;
        newengine_ulog_api::ulog::info!("platform runtime: engine.shutdown begin origin={origin}");
        newengine_core::crash::record_breadcrumb(format!(
            "platform runtime: engine.shutdown begin origin={origin}"
        ));
        match self.engine.shutdown() {
            Ok(()) => {
                newengine_ulog_api::ulog::info!(
                    "platform runtime: engine.shutdown completed origin={origin}"
                );
            }
            Err(e) => {
                newengine_ulog_api::ulog::error!(
                    "platform runtime: engine.shutdown failed origin={origin}: {e}"
                );
            }
        }
        newengine_core::crash::record_breadcrumb(format!(
            "platform runtime: engine.shutdown completed origin={origin}"
        ));
        self.started = false;
        self.shutting_down = false;
    }
}
