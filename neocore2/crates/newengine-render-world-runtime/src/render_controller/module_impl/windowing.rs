#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::host_events::{CursorState, HostEvent, WindowHostEvent, WindowInitSize};
use newengine_core::{EngineResult, ModuleCtx};

use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    #[inline]
    pub(super) fn suspend_zero_sized_surface(&mut self, w: u32, h: u32) {
        if self.viewport.surface_suspended {
            return;
        }

        self.viewport.surface_suspended = true;
        newengine_ulog_api::ulog::info!(
            "render controller: surface suspended because native extent is {}x{}; GPU presentation paused",
            w,
            h
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: surface suspended at native extent {}x{}",
            w, h
        ));
    }

    #[inline]
    pub(super) fn read_window_size<E: Send>(ctx: &ModuleCtx<'_, E>) -> (u32, u32) {
        ctx.resources()
            .get::<WindowInitSize>()
            .map(|s| (s.width, s.height))
            .unwrap_or((0, 0))
    }

    pub(super) fn resize_if_needed(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        w: u32,
        h: u32,
    ) -> EngineResult<()> {
        if w == 0 || h == 0 {
            self.suspend_zero_sized_surface(w, h);
            return Ok(());
        }

        if self.viewport.surface_suspended {
            let old_w = self.viewport.last_w;
            let old_h = self.viewport.last_h;
            newengine_ulog_api::ulog::info!(
                "render controller: restoring presentable surface {}x{} -> {}x{}",
                old_w,
                old_h,
                w,
                h
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: restoring presentable surface {}x{} -> {}x{}",
                old_w, old_h, w, h
            ));

            // Force the backend through its resize/recreate path even when the restored
            // dimensions equal the pre-minimize dimensions. The OS may have invalidated
            // the old swapchain while the native surface had no presentable extent.
            r.resize(w, h)?;

            self.viewport.last_w = w;
            self.viewport.last_h = h;
            self.viewport.surface_suspended = false;
            newengine_ulog_api::ulog::info!(
                "render controller: presentable surface restored {}x{}",
                w,
                h
            );
            return Ok(());
        }

        // The render backend is initialized from the platform window snapshot before the
        // first runtime/game frame. Replaying the initial size through RenderApi::resize
        // can push backend through a redundant swapchain teardown before the first acquire.
        if self.viewport.last_w == 0 || self.viewport.last_h == 0 {
            self.viewport.last_w = w;
            self.viewport.last_h = h;
            newengine_ulog_api::ulog::debug!(
                "render controller: adopted initial surface size {}x{}; skip first explicit resize",
                w,
                h
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: adopted initial surface size {}x{}; skip first resize",
                w, h
            ));
            return Ok(());
        }

        if w != self.viewport.last_w || h != self.viewport.last_h {
            let old_w = self.viewport.last_w;
            let old_h = self.viewport.last_h;
            newengine_ulog_api::ulog::debug!(
                "render controller: resize requested {}x{} -> {}x{}",
                old_w,
                old_h,
                w,
                h
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: resize requested {}x{} -> {}x{}",
                old_w, old_h, w, h
            ));

            r.resize(w, h)?;

            self.viewport.last_w = w;
            self.viewport.last_h = h;
            newengine_ulog_api::ulog::debug!("render controller: resize completed {}x{}", w, h);
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: resize completed {}x{}",
                w, h
            ));
        }
        Ok(())
    }

    #[inline]
    pub(super) fn sync_cursor_state<E: Send>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        desired: CursorState,
    ) {
        self.sync_cursor_state_inner(ctx, desired, false);
    }

    #[inline]
    pub(super) fn force_cursor_state<E: Send>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        desired: CursorState,
    ) {
        self.sync_cursor_state_inner(ctx, desired, true);
    }

    #[inline]
    fn sync_cursor_state_inner<E: Send>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        desired: CursorState,
        force: bool,
    ) {
        if !force && desired == self.viewport.last_cursor_state {
            return;
        }
        self.viewport.last_cursor_state = desired;
        let _ = ctx
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Cursor(desired)));
    }
}
