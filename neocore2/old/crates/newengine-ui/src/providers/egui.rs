#![forbid(unsafe_op_in_unsafe_fn)]

use crate::draw::UiDrawList;
use crate::input::UiInputFrame;
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

    #[inline]
    fn egui_key_from_input(u: u32) -> Option<egui::Key> {
        let backspace = winit::keyboard::KeyCode::Backspace as u32;
        let enter = winit::keyboard::KeyCode::Enter as u32;
        let tab = winit::keyboard::KeyCode::Tab as u32;
        let escape = winit::keyboard::KeyCode::Escape as u32;

        let up = winit::keyboard::KeyCode::ArrowUp as u32;
        let down = winit::keyboard::KeyCode::ArrowDown as u32;
        let left = winit::keyboard::KeyCode::ArrowLeft as u32;
        let right = winit::keyboard::KeyCode::ArrowRight as u32;

        let home = winit::keyboard::KeyCode::Home as u32;
        let end = winit::keyboard::KeyCode::End as u32;
        let page_up = winit::keyboard::KeyCode::PageUp as u32;
        let page_down = winit::keyboard::KeyCode::PageDown as u32;
        let insert = winit::keyboard::KeyCode::Insert as u32;
        let delete = winit::keyboard::KeyCode::Delete as u32;

        Some(match u {
            x if x == backspace => egui::Key::Backspace,
            x if x == enter => egui::Key::Enter,
            x if x == tab => egui::Key::Tab,
            x if x == escape => egui::Key::Escape,

            x if x == up => egui::Key::ArrowUp,
            x if x == down => egui::Key::ArrowDown,
            x if x == left => egui::Key::ArrowLeft,
            x if x == right => egui::Key::ArrowRight,

            x if x == home => egui::Key::Home,
            x if x == end => egui::Key::End,
            x if x == page_up => egui::Key::PageUp,
            x if x == page_down => egui::Key::PageDown,
            x if x == insert => egui::Key::Insert,
            x if x == delete => egui::Key::Delete,

            _ => return None,
        })
    }

    #[inline]
    fn compute_modifiers(input: &UiInputFrame) -> egui::Modifiers {
        let ctrl_l = winit::keyboard::KeyCode::ControlLeft as u32;
        let ctrl_r = winit::keyboard::KeyCode::ControlRight as u32;

        let shift_l = winit::keyboard::KeyCode::ShiftLeft as u32;
        let shift_r = winit::keyboard::KeyCode::ShiftRight as u32;

        let alt_l = winit::keyboard::KeyCode::AltLeft as u32;
        let alt_r = winit::keyboard::KeyCode::AltRight as u32;

        let ctrl = input.is_key_down(ctrl_l) || input.is_key_down(ctrl_r);

        egui::Modifiers {
            alt: input.is_key_down(alt_l) || input.is_key_down(alt_r),
            ctrl,
            shift: input.is_key_down(shift_l) || input.is_key_down(shift_r),
            mac_cmd: false,
            command: ctrl,
        }
    }

    fn inject_input_events(raw: &mut egui::RawInput, input: &UiInputFrame) {
        raw.modifiers = Self::compute_modifiers(input);

        // egui expects positions in "points" (logical units).
        // INPUT plugin usually reports physical pixels.
        let ppp = raw.viewport().native_pixels_per_point.unwrap_or(1.0).max(0.0001);
        let to_pt = |v: f32| v / ppp;

        let mouse_pos_pt = input
            .mouse_pos
            .map(|(x, y)| egui::pos2(to_pt(x), to_pt(y)));

        if let Some(pos) = mouse_pos_pt {
            raw.events.push(egui::Event::PointerMoved(pos));
        }

        let map_btn = |b: u32| -> Option<egui::PointerButton> {
            match b {
                1 => Some(egui::PointerButton::Primary),
                2 => Some(egui::PointerButton::Secondary),
                3 => Some(egui::PointerButton::Middle),
                4 => Some(egui::PointerButton::Extra1),
                5 => Some(egui::PointerButton::Extra2),
                _ => None,
            }
        };

        for &b in input.mouse_pressed.iter() {
            if let Some(btn) = map_btn(b) {
                let pos = mouse_pos_pt.unwrap_or_else(|| egui::pos2(0.0, 0.0));
                raw.events.push(egui::Event::PointerButton {
                    pos,
                    button: btn,
                    pressed: true,
                    modifiers: raw.modifiers,
                });
            }
        }

        for &b in input.mouse_released.iter() {
            if let Some(btn) = map_btn(b) {
                let pos = mouse_pos_pt.unwrap_or_else(|| egui::pos2(0.0, 0.0));
                raw.events.push(egui::Event::PointerButton {
                    pos,
                    button: btn,
                    pressed: false,
                    modifiers: raw.modifiers,
                });
            }
        }

        // Wheel: convert to points as well.
        if input.mouse_wheel.0 != 0.0 || input.mouse_wheel.1 != 0.0 {
            raw.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(to_pt(input.mouse_wheel.0), to_pt(input.mouse_wheel.1)),
                modifiers: raw.modifiers,
            });
        }

        for &k in input.keys_pressed.iter() {
            if let Some(key) = Self::egui_key_from_input(k) {
                raw.events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: raw.modifiers,
                });
            }
        }
        for &k in input.keys_released.iter() {
            if let Some(key) = Self::egui_key_from_input(k) {
                raw.events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: raw.modifiers,
                });
            }
        }

        if !input.text.is_empty() {
            raw.events.push(egui::Event::Text(input.text.clone()));
        }

        if !input.ime_commit.is_empty() {
            raw.events.push(egui::Event::Text(input.ime_commit.clone()));
        }

        if !input.ime_preedit.is_empty() {
            raw.events
                .push(egui::Event::Ime(egui::ImeEvent::Preedit(input.ime_preedit.clone())));
        }
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

    fn on_platform_event(&mut self, _window: &dyn Any, _event: &dyn Any) {
        // HARD NOOP: input must come exclusively from INPUT plugin.
    }

    fn run_frame(
        &mut self,
        window: &dyn Any,
        frame: UiFrameDesc,
        build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput {
        let Some(w) = window.downcast_ref::<winit::window::Window>() else {
            return UiFrameOutput::empty();
        };

        // Base input for screen rect/ppp/time (no events are fed via egui_winit::State).
        let mut raw_input = {
            let state = self.ensure_state(w);
            state.take_egui_input(w)
        };

        // Inject canonical input from INPUT plugin snapshot.
        if let Some(ref input) = frame.input {
            Self::inject_input_events(&mut raw_input, input);
        }

        self.ctx.begin_pass(raw_input);
        build.build(&mut self.ctx);
        let full_output = self.ctx.end_pass();

        {
            let state = self.ensure_state(w);
            state.handle_platform_output(w, full_output.platform_output.clone());
        }

        self.draw_list.clear();
        translate::egui_output_to_draw_list(&self.ctx, full_output, &mut self.draw_list);

        UiFrameOutput {
            draw_list: self.draw_list.clone(),
        }
    }
}