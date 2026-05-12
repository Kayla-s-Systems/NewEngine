#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "content/paths.rs"]
mod paths;
#[path = "content/profile.rs"]
mod profile;

include!("content_parts/raw_payload.rs");
include!("content_parts/profile_parse.rs");
include!("content_parts/sanitize_defaults.rs");
