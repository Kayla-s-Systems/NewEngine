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
        use winit::keyboard::KeyCode as KC;

        // NOTE: The INPUT plugin reports `winit::keyboard::KeyCode` discriminants as `u32`.
        // We translate them into `egui::Key` so UI code can use `ctx.input(|i| i.key_down(..))`.

        Some(match u {
            x if x == (KC::Backspace as u32) => egui::Key::Backspace,
            x if x == (KC::Enter as u32) => egui::Key::Enter,
            x if x == (KC::Tab as u32) => egui::Key::Tab,
            x if x == (KC::Escape as u32) => egui::Key::Escape,
            x if x == (KC::Space as u32) => egui::Key::Space,

            x if x == (KC::ArrowUp as u32) => egui::Key::ArrowUp,
            x if x == (KC::ArrowDown as u32) => egui::Key::ArrowDown,
            x if x == (KC::ArrowLeft as u32) => egui::Key::ArrowLeft,
            x if x == (KC::ArrowRight as u32) => egui::Key::ArrowRight,

            x if x == (KC::Home as u32) => egui::Key::Home,
            x if x == (KC::End as u32) => egui::Key::End,
            x if x == (KC::PageUp as u32) => egui::Key::PageUp,
            x if x == (KC::PageDown as u32) => egui::Key::PageDown,
            x if x == (KC::Insert as u32) => egui::Key::Insert,
            x if x == (KC::Delete as u32) => egui::Key::Delete,

            // Letters (WASD etc.)
            x if x == (KC::KeyA as u32) => egui::Key::A,
            x if x == (KC::KeyB as u32) => egui::Key::B,
            x if x == (KC::KeyC as u32) => egui::Key::C,
            x if x == (KC::KeyD as u32) => egui::Key::D,
            x if x == (KC::KeyE as u32) => egui::Key::E,
            x if x == (KC::KeyF as u32) => egui::Key::F,
            x if x == (KC::KeyG as u32) => egui::Key::G,
            x if x == (KC::KeyH as u32) => egui::Key::H,
            x if x == (KC::KeyI as u32) => egui::Key::I,
            x if x == (KC::KeyJ as u32) => egui::Key::J,
            x if x == (KC::KeyK as u32) => egui::Key::K,
            x if x == (KC::KeyL as u32) => egui::Key::L,
            x if x == (KC::KeyM as u32) => egui::Key::M,
            x if x == (KC::KeyN as u32) => egui::Key::N,
            x if x == (KC::KeyO as u32) => egui::Key::O,
            x if x == (KC::KeyP as u32) => egui::Key::P,
            x if x == (KC::KeyQ as u32) => egui::Key::Q,
            x if x == (KC::KeyR as u32) => egui::Key::R,
            x if x == (KC::KeyS as u32) => egui::Key::S,
            x if x == (KC::KeyT as u32) => egui::Key::T,
            x if x == (KC::KeyU as u32) => egui::Key::U,
            x if x == (KC::KeyV as u32) => egui::Key::V,
            x if x == (KC::KeyW as u32) => egui::Key::W,
            x if x == (KC::KeyX as u32) => egui::Key::X,
            x if x == (KC::KeyY as u32) => egui::Key::Y,
            x if x == (KC::KeyZ as u32) => egui::Key::Z,

            // Digits
            x if x == (KC::Digit0 as u32) => egui::Key::Num0,
            x if x == (KC::Digit1 as u32) => egui::Key::Num1,
            x if x == (KC::Digit2 as u32) => egui::Key::Num2,
            x if x == (KC::Digit3 as u32) => egui::Key::Num3,
            x if x == (KC::Digit4 as u32) => egui::Key::Num4,
            x if x == (KC::Digit5 as u32) => egui::Key::Num5,
            x if x == (KC::Digit6 as u32) => egui::Key::Num6,
            x if x == (KC::Digit7 as u32) => egui::Key::Num7,
            x if x == (KC::Digit8 as u32) => egui::Key::Num8,
            x if x == (KC::Digit9 as u32) => egui::Key::Num9,

            // Function keys
            x if x == (KC::F1 as u32) => egui::Key::F1,
            x if x == (KC::F2 as u32) => egui::Key::F2,
            x if x == (KC::F3 as u32) => egui::Key::F3,
            x if x == (KC::F4 as u32) => egui::Key::F4,
            x if x == (KC::F5 as u32) => egui::Key::F5,
            x if x == (KC::F6 as u32) => egui::Key::F6,
            x if x == (KC::F7 as u32) => egui::Key::F7,
            x if x == (KC::F8 as u32) => egui::Key::F8,
            x if x == (KC::F9 as u32) => egui::Key::F9,
            x if x == (KC::F10 as u32) => egui::Key::F10,
            x if x == (KC::F11 as u32) => egui::Key::F11,
            x if x == (KC::F12 as u32) => egui::Key::F12,

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