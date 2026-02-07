use crate::error::EngineResult;
use crate::module::ModuleCtx;

use std::any::Any;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ApiVersion {
    #[inline]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApiProvide {
    pub id: &'static str,
    pub version: ApiVersion,
}

impl ApiProvide {
    #[inline]
    pub const fn new(id: &'static str, version: ApiVersion) -> Self {
        Self { id, version }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApiRequire {
    pub id: &'static str,
    pub min_version: ApiVersion,
}

impl ApiRequire {
    #[inline]
    pub const fn new(id: &'static str, min_version: ApiVersion) -> Self {
        Self { id, min_version }
    }
}

pub trait Module<E: Send + 'static>: Send {
    fn id(&self) -> &'static str {
        "module"
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &[]
    }

    fn provides(&self) -> &'static [ApiProvide] {
        &[]
    }

    fn requires(&self) -> &'static [ApiRequire] {
        &[]
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

    #[deprecated(note = "Use Engine::emit(...) + EventHub subscriptions instead")]
    fn on_external_event(
        &mut self,
        _ctx: &mut ModuleCtx<'_, E>,
        _event: &dyn Any,
    ) -> EngineResult<()> {
        Ok(())
    }

    fn shutdown(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }
}
