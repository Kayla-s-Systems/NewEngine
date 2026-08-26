#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

// Startup settings facade; fragments share the public settings namespace for API compatibility.
include!("settings/environment_keys.rs");
include!("settings/quality_presets.rs");
include!("settings/display.rs");
include!("settings/graphics.rs");
include!("settings/launch.rs");
include!("settings/active.rs");
include!("settings/tests.rs");
