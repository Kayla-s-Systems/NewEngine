#![forbid(unsafe_op_in_unsafe_fn)]

//! Profile-owned FPS gameplay package.
//!
//! `newengine-engine-runtime` owns generic execution/UI boundaries and shared runtime
//! contracts. This crate owns FPS-only behavior, authored content and HUD policy.

mod character_control;
mod character_physics;
mod combat;
mod content;
mod fps_demo;
mod game_data;
mod inventory_hud;
mod item_assets;
mod projectiles;
mod provider;
mod script_commands;

pub use combat::step_player_combat;
pub use content::{
    default_fps_loadout_id, default_medkit_item_id, default_rifle_ammo_id, default_rifle_item_id,
    FpsContentProvider, DEFAULT_FPS_LOADOUT_NAME, DEFAULT_MEDKIT_ITEM_NAME,
    DEFAULT_RIFLE_AMMO_NAME, DEFAULT_RIFLE_ITEM_NAME,
};
pub use fps_demo::step_fps_demo_gameplay;
pub use inventory_hud::FpsInventoryHudProvider;
pub use item_assets::{
    compile_authored_item_package, decode_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8, install_compiled_item_package,
    parse_authored_item_package_json, AuthoredItemDefinition, AuthoredItemPackage,
    AuthoredLoadoutDefinition, AuthoredLoadoutEntry, AuthoredUseEffect, AuthoredWeaponDefinition,
    CompiledItemPackage, AUTHORED_ITEM_PACKAGE_SCHEMA, AUTHORED_ITEM_PACKAGE_VERSION,
    NEITEMS_LOGICAL_PATH,
};
pub use newengine_gameplay_fps_api::{
    action as fps_action, FpsActionFrame, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules,
    FpsDemoState, FpsDemoTarget, FpsPlayerTuning,
};
pub use projectiles::{
    spawn_projectile_sphere, step_projectile_sphere_launcher, ProjectileSphereRuntime,
    ProjectileSphereTuning,
};
pub use provider::FpsGameplayProvider;

#[cfg(test)]
mod inventory_tests;
