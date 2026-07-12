#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted baseline provider for the `engine.time` gateway.

mod constants;
mod controls;
mod derived;
mod events;
mod frame;
mod global;
mod invoke;
mod registration;
mod router;
mod state;

pub use registration::register_time_gateway_best_effort;

#[cfg(test)]
mod tests;
