#![forbid(unsafe_op_in_unsafe_fn)]

//! Declarative host-side app launcher.
//!
//! Apps describe product identity and runtime profile; the host owns bootstrap,
//! plugin composition, platform policy, logging and process shutdown.

mod boot_options;
mod bootstrap;
mod logging;
mod plugins;
mod project_content;
mod runtime_units;
mod shutdown;
mod types;

pub use boot_options::RuntimeHostBootOption;
pub use types::{
    RuntimeHostAppProfile, RuntimeHostFrontend, RuntimeHostFrontendContext, RuntimeHostLaunchSpec,
    RuntimeHostLauncher, RuntimeHostRuntimeUnitRegistration, RuntimeUnitCompositionReport,
    RuntimeUnitFactory,
};

#[inline]
fn env_bool(name: &str, default: bool) -> bool {
    newengine_plugin_host::current_host_context()
        .environment_var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
