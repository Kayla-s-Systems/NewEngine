use crate::draw::UiDrawList;
use crate::provider::{UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind};
use std::any::Any;

mod translate;

pub struct EguiUiProvider {
    ctx: egui::Context,
    state: Option<egui_winit::State>,
    draw_list: UiDrawList,
}

impl EguiUiProvider {
    #[inline]
    pub fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            state: None,
            draw_list: UiDrawList::new(),
        }
    }

    #[inline]
    fn ensure_state(&mut self, window: &winit::window::Window) -> &mut egui_winit::State {
        if self.state.is_none() {
            let s = egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                window,
                Some(window.scale_factor() as f32),
                None,
                None,
            );
            self.state = Some(s);
        }
        self.state.as_mut().unwrap()
    }
}

impl UiProvider for EguiUiProvider {
    #[inline]
    fn kind(&self) -> UiProviderKind {
        UiProviderKind::Egui
    }

    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn on_platform_event(&mut self, window: &dyn Any, event: &dyn Any) {
        let Some(w) = window.downcast_ref::<winit::window::Window>() else { return };
        let Some(ev) = event.downcast_ref::<winit::event::WindowEvent>() else { return };

        let state = self.ensure_state(w);
        let _ = state.on_window_event(w, ev);
    }

    fn run_frame(
        &mut self,
        window: &dyn Any,
        _frame: UiFrameDesc,
        build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput {
        let Some(w) = window.downcast_ref::<winit::window::Window>() else {
            return UiFrameOutput::empty();
        };

        // 1) Take raw input with a short-lived mutable borrow of state.
        let raw_input = {
            let state = self.ensure_state(w);
            state.take_egui_input(w)
        };

        // 2) Run egui pass (only ctx borrow now).
        self.ctx.begin_pass(raw_input);
        build.build(&mut self.ctx);
        let full_output = self.ctx.end_pass();

        // 3) Handle platform output with another short-lived mutable borrow of state.
        {
            let state = self.ensure_state(w);
            state.handle_platform_output(w, full_output.platform_output.clone());
        }

        // 4) Translate output into draw list (immutable ctx is fine now).
        self.draw_list.clear();
        translate::egui_output_to_draw_list(&self.ctx, full_output, &mut self.draw_list);

        UiFrameOutput {
            draw_list: self.draw_list.clone(),
        }
    }
}