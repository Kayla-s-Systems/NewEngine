use newengine_core::EngineError;

use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    /// Run the app and terminate the process with the correct code.
    pub fn run_process(self) -> ! {
        self.prepare_early_log_session();
        self.early_log(format_args!(
            "process.entry exe={:?} cwd={:?}",
            std::env::current_exe().ok(),
            std::env::current_dir().ok()
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: main entry",
            self.spec.app_name
        ));

        match self.run() {
            Ok(()) | Err(EngineError::ExitRequested) => {
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: exit requested",
                    self.spec.app_name
                ));
                std::process::exit(0);
            }
            Err(e) => {
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: fatal error='{}'",
                    self.spec.app_name, e
                ));
                let report = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
                match report {
                    Some(path) => newengine_ulog_api::ulog::error!(
                        "{} launcher fatal: {} | crash_report='{}'",
                        self.spec.app_name,
                        e,
                        path.display()
                    ),
                    None => newengine_ulog_api::ulog::error!(
                        "{} launcher fatal: {e}",
                        self.spec.app_name
                    ),
                }
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
