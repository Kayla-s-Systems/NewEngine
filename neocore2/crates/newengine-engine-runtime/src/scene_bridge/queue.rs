#![forbid(unsafe_op_in_unsafe_fn)]

use super::SceneCommand;

#[derive(Default)]
pub(super) struct SceneQueue {
    pub(super) cmds: Vec<SceneCommand>,
}
