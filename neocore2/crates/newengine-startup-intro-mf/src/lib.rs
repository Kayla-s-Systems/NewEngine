#![forbid(unsafe_op_in_unsafe_fn)]

/// Installs the Windows Media Foundation presenter into the platform-neutral
/// startup-intro presenter port. Registration is deterministic and first-wins.
#[cfg(windows)]
pub fn install() -> bool {
    newengine_startup_intro::install_startup_intro_presenter(windows_provider::present)
}

#[cfg(not(windows))]
pub fn install() -> bool {
    false
}

#[cfg(windows)]
mod windows_provider;
