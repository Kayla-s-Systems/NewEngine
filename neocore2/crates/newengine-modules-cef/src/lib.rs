mod api;
mod content;
mod runtime;

pub use api::{CefApi, CefApiRef, CefViewId};
pub use content::{
    CefContentApi, CefContentApiRef, CefContentModule, CefContentRequest, CefHttpRequest,
};
use runtime::CefRuntime;

use newengine_core::{EngineResult, Module, ModuleCtx, WindowHostEvent};

pub struct CefModule {
    rt: Option<CefRuntime>,
}

impl CefModule {
    #[inline]
    pub fn new() -> Self {
        Self { rt: None }
    }
}

impl<E: Send + 'static> Module<E> for CefModule {
    fn id(&self) -> &'static str {
        "cef"
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let rt = CefRuntime::new().map_err(|e| newengine_core::EngineError::Other(e))?;
        let api = rt.api();

        ctx.resources().insert::<CefApiRef>(api);

        self.rt = Some(rt);
        Ok(())
    }

    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if let Some(rt) = &mut self.rt {
            rt.tick();
        }
        Ok(())
    }

    fn on_external_event(
        &mut self,
        _ctx: &mut ModuleCtx<'_, E>,
        event: &dyn std::any::Any,
    ) -> EngineResult<()> {
        let Some(ev) = event.downcast_ref::<WindowHostEvent>() else {
            return Ok(());
        };

        let Some(rt) = &mut self.rt else {
            return Ok(());
        };

        match *ev {
            WindowHostEvent::Ready {
                window,
                display,
                width,
                height,
            } => {
                rt.attach_window(window, display, width, height)
                    .map_err(|e| newengine_core::EngineError::Other(e))?;
            }
            WindowHostEvent::Resized { width, height } => {
                rt.resize(width, height);
            }
            WindowHostEvent::Focused(focused) => {
                rt.focus(focused);
            }
        }

        Ok(())
    }

    fn shutdown(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if let Some(rt) = &mut self.rt {
            rt.shutdown();
        }
        self.rt = None;
        Ok(())
    }
}
