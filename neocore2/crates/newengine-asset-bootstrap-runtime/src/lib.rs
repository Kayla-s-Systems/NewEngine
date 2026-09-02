#![forbid(unsafe_op_in_unsafe_fn)]

mod log_path;
mod mounts;
mod roots;
mod window_icon;

pub use log_path::shard_log_path_by_run_id;
pub use mounts::{mount_asset_roots_best_effort, mount_profile_content_best_effort};
pub use roots::{
    collect_app_asset_roots, collect_profile_mount_roots, ContentSetSpec, ProfileMountSpec,
};
pub use window_icon::try_load_window_icon_best_effort;
