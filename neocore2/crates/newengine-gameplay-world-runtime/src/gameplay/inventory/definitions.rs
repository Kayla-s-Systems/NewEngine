use super::*;

// Inventory definition domains stay in one Rust module so public type paths remain stable while
// physical ownership is split by responsibility.
include!("definitions/core.rs");
include!("definitions/world.rs");
include!("definitions/weapon_profiles.rs");
include!("definitions/weapon_stats.rs");
include!("definitions/weapon_core.rs");
include!("definitions/weapon_presentation.rs");
include!("definitions/ammo.rs");
include!("definitions/components.rs");
include!("definitions/item.rs");
