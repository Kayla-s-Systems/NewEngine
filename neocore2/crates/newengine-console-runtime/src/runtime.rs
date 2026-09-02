#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_console_api::{CommandDescriptor, CommandFlags, ENGINE_COMMAND_GATEWAY_ID};
use newengine_core::{call_service_v1, describe_service, list_service_ids, ShutdownToken};
use newengine_plugin_host::services_generation;

use super::cvar::{global_cvar_registry, CVarRegistry, CVarSnapshot};
use super::types::{ConsoleCmdEntry, DynCommand, DynPayload, SuggestItem, SuggestResponse};

use newengine_math::collections_prelude::NeBTreeMap as BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

type CmdFn = fn(&ConsoleRuntime, &str) -> Result<String, String>;

struct Cmd {
    help: &'static str,
    usage: &'static str,
    f: CmdFn,
}

pub struct ConsoleRuntime {
    cmds: BTreeMap<&'static str, Cmd>,

    dyn_cmds: Mutex<BTreeMap<String, DynCommand>>,
    method_cache: Mutex<BTreeMap<String, Vec<String>>>,
    cvars: Arc<CVarRegistry>,

    cached_services_gen: AtomicU64,
}

include!("runtime/construction.rs");
include!("runtime/completion.rs");
include!("runtime/suggest.rs");
include!("runtime/dynamic.rs");
include!("runtime/cvars.rs");
include!("runtime/services.rs");

impl ConsoleRuntime {
    #[allow(dead_code)]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
}
