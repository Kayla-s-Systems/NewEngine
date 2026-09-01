// Physical source slices share this crate facade so public symbols and ABI stay unchanged.
include!("weapon_fx/event_routing.rs");
include!("weapon_fx/impact_debris.rs");
include!("weapon_fx/contact_physics.rs");
include!("weapon_fx/shot_spawning.rs");
include!("weapon_fx/hit_resolution.rs");
