#![forbid(unsafe_op_in_unsafe_fn)]

use crate::provider::{UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind};
use std::any::Any;

pub struct NullUiProvider;

impl NullUiProvider {
    #[inline]
    pub fn new() -> Self {
        Self
    }
}

impl UiProvider for NullUiProvider {
    #[inline]
    fn kind(&self) -> UiProviderKind {
        UiProviderKind::Null
    }

    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn run_frame(
        &mut self,
        _window: &dyn Any,
        _frame: UiFrameDesc,
        _build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput {
        UiFrameOutput::empty()
    }
}