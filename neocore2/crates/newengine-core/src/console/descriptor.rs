#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgSpec {
    pub name: String,
    #[serde(default)]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFlags {
    #[serde(default)]
    pub developer: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub remote_allowed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDescriptor {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub usage: String,
    #[serde(default)]
    pub args: Vec<CommandArgSpec>,
    #[serde(default)]
    pub flags: CommandFlags,
    #[serde(default)]
    pub owner: String,
}
