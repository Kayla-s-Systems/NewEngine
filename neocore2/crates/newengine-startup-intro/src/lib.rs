#![forbid(unsafe_op_in_unsafe_fn)]

mod manifest;
mod model;
mod presenter;
mod resolver;

pub use manifest::{
    StartupIntroEntry, StartupIntroManifest, StartupIntroWindow, STARTUP_INTRO_SCHEMA,
    STARTUP_INTRO_SKIP_ENV,
};
pub use model::{
    ResolvedStartupIntro, ResolvedStartupIntroEntry, ResolvedStartupIntroWindow,
    StartupIntroNativeBackend, StartupIntroNativeWindow, StartupIntroReport, StartupIntroStatus,
};
pub use presenter::{
    install_startup_intro_presenter, play_from_descriptor_in_window,
    startup_intro_presenter_registered, StartupIntroPresenterFn,
};
pub use resolver::resolve_descriptor_path;

#[cfg(test)]
mod tests;
