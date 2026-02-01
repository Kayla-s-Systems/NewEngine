mod error;
mod render_api;
mod vulkan;

use newengine_core::render::{BeginFrameDesc, RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};

use newengine_platform_winit::{WinitWindowHandles, WinitWindowInitSize};

use crate::error::VkRenderError;
use crate::render_api::VulkanRenderApi;

pub struct VulkanAshRenderModule {
    api: Option<RenderApiRef>,
    last_w: u32,
    last_h: u32,
}

impl Default for VulkanAshRenderModule {
    fn default() -> Self {
        Self {
            api: None,
            last_w: 0,
            last_h: 0,
        }
    }
}

impl<E: Send + 'static> Module<E> for VulkanAshRenderModule {
    fn id(&self) -> &'static str {
        "render.vulkan.ash"
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[RENDER_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let (display, window, w, h) = {
            let handles = ctx
                .resources()
                .get::<WinitWindowHandles>()
                .ok_or_else(|| EngineError::other(VkRenderError::MissingWindowHandles.to_string()))?;

            let size = ctx
                .resources()
                .get::<WinitWindowInitSize>()
                .ok_or_else(|| EngineError::other(VkRenderError::MissingWindowSize.to_string()))?;

            (handles.display, handles.window, size.width, size.height)
        };

        let renderer = unsafe { vulkan::VulkanRenderer::new(display, window, w, h) }
            .map_err(|e| EngineError::other(e.to_string()))?;

        let api = RenderApiRef::new(VulkanRenderApi::new(renderer, w, h));

        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;

        self.api = Some(api);
        self.last_w = w;
        self.last_h = h;

        Ok(())
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let Some(api) = self.api.as_ref() else {
            return Ok(());
        };

        let (w, h) = ctx
            .resources()
            .get::<WinitWindowInitSize>()
            .map(|s| (s.width, s.height))
            .unwrap_or((0, 0));

        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            api.lock().resize(w, h)?;
        }

        {
            let mut r = api.lock();
            r.begin_frame(BeginFrameDesc::new([0.0, 0.0, 0.0, 1.0]))?;
            r.end_frame()?;
        }

        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx
            .resources_mut()
            .unregister_api::<RenderApiRef>(RENDER_API_ID);
        self.api = None;
        Ok(())
    }
}