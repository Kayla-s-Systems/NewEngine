use crate::draw::UiDrawList;
use std::any::Any;

/// Frame descriptor (extend later as needed).
#[derive(Debug, Clone, Copy)]
pub struct UiFrameDesc {
    pub dt_sec: f32,
}

impl UiFrameDesc {
    #[inline]
    pub const fn new(dt_sec: f32) -> Self {
        Self { dt_sec }
    }
}

/// Output of a UI frame.
#[derive(Debug, Clone)]
pub struct UiFrameOutput {
    pub draw_list: UiDrawList,
}

impl UiFrameOutput {
    #[inline]
    pub fn empty() -> Self {
        Self {
            draw_list: UiDrawList::new(),
        }
    }
}

/// Object-safe UI build callback.
/// Providers may expose a typed context via `ctx_any`; callers can downcast.
pub trait UiBuildFn {
    fn build(&mut self, ctx_any: &mut dyn Any);
}

/// Provider kind selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiProviderKind {
    Null,
    Egui,
}

/// Provider creation options.
#[derive(Debug, Clone)]
pub struct UiProviderOptions {
    pub kind: UiProviderKind,
}

impl Default for UiProviderOptions {
    #[inline]
    fn default() -> Self {
        Self {
            kind: UiProviderKind::Egui,
        }
    }
}

/// Replaceable UI provider interface.
///
/// The trait is platform-agnostic by design:
/// - window and events are passed as `dyn Any`
/// - concrete provider decides what it supports
pub trait UiProvider: Send {
    fn kind(&self) -> UiProviderKind;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Feed platform event (optional).
    fn on_platform_event(&mut self, _window: &dyn Any, _event: &dyn Any) {}

    /// Run one UI frame.
    fn run_frame(
        &mut self,
        window: &dyn Any,
        frame: UiFrameDesc,
        build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput;
}