use std::{env, path::Path, sync::OnceLock};

use crate::model::{
    ResolvedStartupIntro, StartupIntroNativeWindow, StartupIntroReport, StartupIntroStatus,
};
use crate::resolver::{load_manifest, resolve_payload};
use crate::STARTUP_INTRO_SKIP_ENV;

/// Host/provider presentation port. The contract crate owns sequencing and validation.
/// Presentation always targets an already-created game window; providers must never
/// create a second splash window.
pub type StartupIntroPresenterFn =
    fn(&ResolvedStartupIntro, StartupIntroNativeWindow) -> Result<(), String>;

static STARTUP_INTRO_PRESENTER: OnceLock<StartupIntroPresenterFn> = OnceLock::new();

/// Installs the process startup-intro presenter. Registration is deterministic and first-wins.
pub fn install_startup_intro_presenter(presenter: StartupIntroPresenterFn) -> bool {
    STARTUP_INTRO_PRESENTER.set(presenter).is_ok()
}

#[inline]
pub fn startup_intro_presenter_registered() -> bool {
    STARTUP_INTRO_PRESENTER.get().is_some()
}

pub fn play_from_descriptor_in_window(
    descriptor_path: impl AsRef<Path>,
    root_dir: impl AsRef<Path>,
    target: StartupIntroNativeWindow,
) -> StartupIntroReport {
    let descriptor_path = descriptor_path.as_ref().to_path_buf();

    if env_bool(STARTUP_INTRO_SKIP_ENV, false) {
        return StartupIntroReport::new(
            StartupIntroStatus::Skipped,
            descriptor_path,
            0,
            format!("startup intro suppressed by {STARTUP_INTRO_SKIP_ENV}"),
        );
    }

    let manifest = match load_manifest(&descriptor_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return StartupIntroReport::new(
                StartupIntroStatus::Unavailable,
                descriptor_path,
                0,
                error,
            )
        }
    };
    if !manifest.enabled {
        return StartupIntroReport::new(
            StartupIntroStatus::Disabled,
            descriptor_path,
            0,
            "startup intro descriptor is disabled",
        );
    }

    let payload = match resolve_payload(&manifest, &descriptor_path, root_dir.as_ref()) {
        Ok(payload) => payload,
        Err(error) => {
            return StartupIntroReport::new(
                StartupIntroStatus::Unavailable,
                descriptor_path,
                0,
                error,
            )
        }
    };
    if payload.sequence.is_empty() {
        return StartupIntroReport::new(
            StartupIntroStatus::Empty,
            descriptor_path,
            0,
            "startup intro sequence has no enabled entries",
        );
    }

    let entry_count = payload.sequence.len();
    let Some(presenter) = STARTUP_INTRO_PRESENTER.get().copied() else {
        return StartupIntroReport::new(
            StartupIntroStatus::Unavailable,
            descriptor_path,
            entry_count,
            "startup intro was requested, but no presenter provider is registered",
        );
    };
    match presenter(&payload, target) {
        Ok(()) => StartupIntroReport::new(
            StartupIntroStatus::Played,
            descriptor_path,
            entry_count,
            format!(
                "played {entry_count} startup intro entr{}",
                if entry_count == 1 { "y" } else { "ies" }
            ),
        ),
        Err(error) => StartupIntroReport::new(
            StartupIntroStatus::Unavailable,
            descriptor_path,
            entry_count,
            error,
        ),
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        })
        .unwrap_or(default)
}
