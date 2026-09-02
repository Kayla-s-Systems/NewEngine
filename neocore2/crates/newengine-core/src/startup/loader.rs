#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use crate::startup::config::StartupPluginOverride;
use crate::startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport, StartupOverride,
    StartupResolvedFrom, StartupStorageRootKind, WindowPlacement,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[path = "loader/persistence.rs"]
mod persistence;

include!("loader/load.rs");
include!("loader/dto.rs");
include!("loader/apply.rs");
include!("loader/process_overrides.rs");
include!("loader/storage.rs");
include!("loader/plugin_overrides.rs");
include!("loader/value_apply.rs");
include!("loader/resolve.rs");
