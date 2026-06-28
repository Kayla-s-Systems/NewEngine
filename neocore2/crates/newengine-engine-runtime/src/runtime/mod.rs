//! Runtime orchestration split introduced by Patch 8.
//!
//! The runtime is the conductor: it extracts state, builds DTOs, calls gateway
//! adapters and applies outputs. Providers own implementation details; asset
//! data describes the score.

pub mod adapters;
pub mod definition_apply;
pub mod diagnostics;
pub mod ecs_apply;
pub mod frame;
