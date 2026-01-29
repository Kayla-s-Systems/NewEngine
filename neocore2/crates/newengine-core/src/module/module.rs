use super::ctx::ModuleCtx;
use crate::error::EngineResult;

use std::any::Any;

/// A single engine module.
pub trait Module<E: Send + 'static>: Send {
    fn id(&self) -> &'static str {
        "module"
    }

    fn init(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn fixed_update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn render(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn on_external_event(&mut self, _ctx: &mut ModuleCtx<'_, E>, _event: &dyn Any) -> EngineResult<()> {
        Ok(())
    }

    fn shutdown(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }
}