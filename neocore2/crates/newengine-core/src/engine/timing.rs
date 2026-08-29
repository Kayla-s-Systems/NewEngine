use super::Engine;

use std::fmt;
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Debug)]
pub(super) struct Elapsed {
    value: u128,
    unit: &'static str,
}

impl Elapsed {
    #[inline]
    fn from_duration(d: Duration) -> Self {
        let us = d.as_micros();
        if us < 1000 {
            Self {
                value: us,
                unit: "us",
            }
        } else {
            Self {
                value: d.as_millis(),
                unit: "ms",
            }
        }
    }
}

impl fmt::Display for Elapsed {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "elapsed_{}={}", self.unit, self.value)
    }
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(super) fn elapsed_since(t0: Instant) -> Elapsed {
        Elapsed::from_duration(t0.elapsed())
    }

    #[inline]
    pub(super) fn log_phase_ok(
        scope: &'static str,
        phase: &'static str,
        count: Option<usize>,
        elapsed: Elapsed,
    ) {
        match count {
            Some(n) => {
                newengine_ulog_api::ulog::info!("{scope}: done (phase={phase} count={n} {elapsed})")
            }
            None => newengine_ulog_api::ulog::info!("{scope}: done (phase={phase} {elapsed})"),
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(super) fn phase_err(
        phase: &'static str,
        elapsed: Elapsed,
        e: impl fmt::Display,
    ) -> crate::error::EngineError {
        crate::error::EngineError::Other(format!("plugins: failed (phase={phase} {elapsed}): {e}"))
    }
}
