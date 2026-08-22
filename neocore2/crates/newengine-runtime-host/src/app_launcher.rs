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
mod shutdown;
mod types;

pub use boot_options::RuntimeHostBootOption;
pub use types::{
    RuntimeHostAppProfile, RuntimeHostFrontend, RuntimeHostFrontendContext, RuntimeHostLaunchSpec,
    RuntimeHostLauncher,
};

#[inline]
fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
