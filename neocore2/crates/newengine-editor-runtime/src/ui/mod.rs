#![forbid(unsafe_op_in_unsafe_fn)]

mod actions;
mod camera;
mod commands;
mod dock;
mod layout;
mod icons;
mod logic;
mod panels;
mod shell;
mod property_grid;
mod providers;
pub mod schema;
pub mod extension_abi;
mod state;
mod theme;
mod util;
mod widgets;

pub(crate) use state::*;

use std::any::Any;

use newengine_ui::{UiBuildFn, UiFrameDesc};

impl UiBuildFn for EditorUiBuild {
    #[inline]
    fn begin_frame(&mut self, frame: &UiFrameDesc) {
        self.frame_input = frame.input.clone().unwrap_or_default();
    }

    #[inline]
    fn build(&mut self, ctx_any: &mut dyn Any) {
        self.build_ui(ctx_any);
    }
}
