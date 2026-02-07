#![forbid(unsafe_op_in_unsafe_fn)]

pub const COMMAND_SERVICE_ID: &str = "engine.command";

pub mod method {
    pub const EXEC: &str = "command.exec";
    pub const COMPLETE: &str = "command.complete";
    pub const SUGGEST: &str = "command.suggest";
    pub const REFRESH: &str = "command.refresh";
}