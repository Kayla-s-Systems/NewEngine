//! Runtime-unit composition facade.
//!
//! Inventory assembly, deterministic solving and engine materialization are kept
//! separate so project/runtime profiles remain declarative composition inputs.

mod catalog;
mod materialize;
mod solver;

pub(super) use materialize::materialize_runtime_units;

#[cfg(test)]
mod tests;
