#![forbid(unsafe_op_in_unsafe_fn)]

//! Deterministic replay coordinator primitives.
//!
//! The replay layer is intentionally modeled as a small finite-state machine.
//! Runtime/editor code observes snapshots and submits explicit intents; it must
//! not mirror replay state with ad-hoc booleans such as `is_baking`,
//! `pending_cleanup` or `paused_called`.

mod clock;
mod coordinator;
mod state;
mod transition;

pub use clock::{ReplayJumpTarget, ReplayPlaybackClock, ReplayPlaybackClockSnapshot};
pub use coordinator::{ReplayCoordinatorFsm, ReplayCoordinatorSnapshot};
pub use state::ReplayCoordinatorState;
pub use transition::ReplayCoordinatorTransition;

#[cfg(test)]
mod tests;
