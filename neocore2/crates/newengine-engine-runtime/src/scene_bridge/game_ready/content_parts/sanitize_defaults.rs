use super::*;

#[path = "sanitize_defaults/defaults_core.rs"]
mod defaults_core;
#[path = "sanitize_defaults/defaults_gameplay.rs"]
mod defaults_gameplay;
#[path = "sanitize_defaults/defaults_material.rs"]
mod defaults_material;
#[path = "sanitize_defaults/defaults_sky.rs"]
mod defaults_sky;
#[path = "sanitize_defaults/defaults_terrain.rs"]
mod defaults_terrain;
#[path = "sanitize_defaults/environment.rs"]
mod environment;
#[path = "sanitize_defaults/material.rs"]
mod material;
#[path = "sanitize_defaults/mission.rs"]
mod mission;
#[path = "sanitize_defaults/payload.rs"]
mod payload;

pub(super) use defaults_core::*;
pub(super) use defaults_gameplay::*;
pub(super) use defaults_material::*;
pub(super) use defaults_sky::*;
pub(super) use defaults_terrain::*;
pub(super) use environment::*;
pub(super) use material::*;
pub(super) use mission::*;
pub(super) use payload::*;
