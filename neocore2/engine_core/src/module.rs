use crate::{frame::FrameContext, phase::FramePhase};

/// Модуль движка.
/// Важно: ядро не знает конкретных модулей.
/// Модуль выбирается конфигом и создаётся фабрикой.
pub trait Module {
    fn id(&self) -> &'static str;

    fn on_register(&mut self, _ctx: &mut FrameContext<'_>) {}
    fn on_start(&mut self, _ctx: &mut FrameContext<'_>) {}
    fn on_phase(&mut self, _phase: FramePhase, _ctx: &mut FrameContext<'_>) {}
    fn on_shutdown(&mut self, _ctx: &mut FrameContext<'_>) {}
}