#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use newengine_gameplay_script_api::GameplayCommandBuffer;
use serde::{Deserialize, Serialize};

pub const FPS_GAMEPLAY_POLICY_SCHEMA: &str = "newengine.gameplay.fps.policy.v1";
pub const FPS_GAMEPLAY_POLICY_VERSION: u32 = 1;

pub const FPS_CHARACTER_MENU_POLICY_SCHEMA: &str = "newengine.gameplay.fps.character_menu.v1";
pub const FPS_CHARACTER_MENU_POLICY_VERSION: u32 = 1;

include!("policy/menu.rs");
include!("policy/snapshot.rs");
include!("policy/character.rs");
include!("policy/runtime.rs");
include!("policy/events.rs");

#[cfg(test)]
#[path = "policy/tests.rs"]
mod tests;
