#![forbid(unsafe_op_in_unsafe_fn)]

pub mod ytyp_metadata;

#[path = "content/paths.rs"]
mod paths;
#[path = "content/profile.rs"]
mod profile;
#[path = "content_parts/raw_payload.rs"]
mod raw_payload;

pub use self::profile::*;
pub use self::raw_payload::{
    load_authored_world_profile, load_authored_world_profile_from_resolved_map,
};
