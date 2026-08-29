#![forbid(unsafe_op_in_unsafe_fn)]

//! Lua-backed [`newengine_game_data::GameDataProvider`] adapter.
//!
//! This crate deliberately depends on the generic `engine.scripting` gateway,
//! never on `mlua` or a concrete scripting plugin. The active scripting backend
//! may be Lua today and another implementation later without changing gameplay.

mod client;
mod provider;

pub use provider::{LuaGameDataProvider, LUA_GAME_DATA_PROVIDER_ID, SCRIPT_GAME_DATA_PROVIDER_ID};
