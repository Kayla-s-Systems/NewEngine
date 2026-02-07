#![forbid(unsafe_op_in_unsafe_fn)]

/// Runtime checks for the NewEngine core invariants.
///
/// The core treats invariant violations as fatal, because silent corruption is worse than a crash.
/// This module must remain tiny and dependency-free.

#[cold]
#[inline(never)]
fn violation(msg: &'static str) -> ! {
    panic!("CORE INVARIANT VIOLATION: {msg}");
}

/// Panic if a required invariant is false.
#[inline]
pub fn require(cond: bool, msg: &'static str) {
    if !cond {
        violation(msg);
    }
}

/// Panic if an impossible state transition happens.
#[inline]
pub fn bad_state(msg: &'static str) -> ! {
    violation(msg)
}