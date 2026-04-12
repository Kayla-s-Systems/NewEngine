#![forbid(unsafe_op_in_unsafe_fn)]

mod bus;
mod routing;

pub(crate) use bus::{EditorCommandBus, TypedEditorCommand};
