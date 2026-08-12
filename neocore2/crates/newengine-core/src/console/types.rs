#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

use super::descriptor::{CommandArgSpec, CommandFlags};

#[derive(Debug, Clone, Deserialize)]
pub struct ConsoleCmdEntry {
    pub name: String,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub usage: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub service_id: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub args: Vec<CommandArgSpec>,
    #[serde(default)]
    pub flags: CommandFlags,
    #[serde(default)]
    pub owner: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DynCommand {
    pub help: String,
    pub usage: String,
    pub service_id: String,
    pub method: String,
    pub payload: DynPayload,
    pub args: Vec<CommandArgSpec>,
    pub flags: CommandFlags,
    pub owner: String,
}

#[derive(Debug, Clone, Copy)]
pub enum DynPayload {
    Empty,
    Raw,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestItem {
    pub kind: String,
    pub display: String,
    pub insert: String,
    pub help: String,
    pub usage: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestResponse {
    pub signature: String,
    pub items: Vec<SuggestItem>,
}
