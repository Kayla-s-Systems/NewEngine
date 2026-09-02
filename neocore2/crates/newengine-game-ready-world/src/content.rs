#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "content/paths.rs"]
mod paths;
#[path = "content/profile.rs"]
mod profile;
#[path = "content_parts/raw_payload.rs"]
mod raw_payload;

pub(super) use self::profile::*;
pub(super) use self::raw_payload::load_authored_world_profile;
