use newengine_core::host_events::{
    HostEvent, InputHostEvent, KeyCode, KeyState, MouseButton, TextHostEvent, WindowHostEvent,
};
use newengine_core::{Engine, EngineError, EngineResult};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, Ime, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode as WKeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Engine-thread local window handles (not Send/Sync on some platforms).
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct WinitWindowHandles {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

struct App<E: Send + 'static> {
    engine: Engine<E>,
    window: Option<Window>,
    last_cursor_pos: Option<(f32, f32)>,
}

impl<E: Send + 'static> App<E> {
    #[inline]
    fn new(engine: Engine<E>) -> Self {
        Self {
            engine,
            window: None,
            last_cursor_pos: None,
        }
    }

    #[inline]
    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    #[inline]
    fn exit(event_loop: &ActiveEventLoop) {
        event_loop.exit();
    }

    #[inline]
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window.as_ref().map(|w| {
            let PhysicalSize { width, height } = w.inner_size();
            (width, height)
        })
    }

    fn install_window_handles_resource(&mut self) {
        let Some(w) = &self.window else { return };

        let window = match w.window_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return,
        };

        let display = match w.display_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return,
        };

        self.engine
            .resources_mut()
            .insert(WinitWindowHandles { window, display });
    }

    fn emit_ready(&mut self) {
        let Some((width, height)) = self.window_size() else { return };

        let _ = self.engine
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Ready { width, height }));
    }

    #[inline]
    fn emit_resized(&mut self, width: u32, height: u32) {
        let _ = self.engine.emit(HostEvent::Window(WindowHostEvent::Resized { width, height }));
    }

    #[inline]
    fn emit_focused(&mut self, focused: bool) {
        let _ = self.engine.emit(HostEvent::Window(WindowHostEvent::Focused(focused)));
    }

    #[inline]
    fn map_key_code(code: WKeyCode) -> KeyCode {
        match code {
            WKeyCode::Escape => KeyCode::Escape,
            WKeyCode::Enter => KeyCode::Enter,
            WKeyCode::Space => KeyCode::Space,
            WKeyCode::Tab => KeyCode::Tab,
            WKeyCode::Backspace => KeyCode::Backspace,

            WKeyCode::ArrowUp => KeyCode::ArrowUp,
            WKeyCode::ArrowDown => KeyCode::ArrowDown,
            WKeyCode::ArrowLeft => KeyCode::ArrowLeft,
            WKeyCode::ArrowRight => KeyCode::ArrowRight,

            WKeyCode::KeyA => KeyCode::A,
            WKeyCode::KeyB => KeyCode::B,
            WKeyCode::KeyC => KeyCode::C,
            WKeyCode::KeyD => KeyCode::D,
            WKeyCode::KeyE => KeyCode::E,
            WKeyCode::KeyF => KeyCode::F,
            WKeyCode::KeyG => KeyCode::G,
            WKeyCode::KeyH => KeyCode::H,
            WKeyCode::KeyI => KeyCode::I,
            WKeyCode::KeyJ => KeyCode::J,
            WKeyCode::KeyK => KeyCode::K,
            WKeyCode::KeyL => KeyCode::L,
            WKeyCode::KeyM => KeyCode::M,
            WKeyCode::KeyN => KeyCode::N,
            WKeyCode::KeyO => KeyCode::O,
            WKeyCode::KeyP => KeyCode::P,
            WKeyCode::KeyQ => KeyCode::Q,
            WKeyCode::KeyR => KeyCode::R,
            WKeyCode::KeyS => KeyCode::S,
            WKeyCode::KeyT => KeyCode::T,
            WKeyCode::KeyU => KeyCode::U,
            WKeyCode::KeyV => KeyCode::V,
            WKeyCode::KeyW => KeyCode::W,
            WKeyCode::KeyX => KeyCode::X,
            WKeyCode::KeyY => KeyCode::Y,
            WKeyCode::KeyZ => KeyCode::Z,

            WKeyCode::Digit0 => KeyCode::Digit0,
            WKeyCode::Digit1 => KeyCode::Digit1,
            WKeyCode::Digit2 => KeyCode::Digit2,
            WKeyCode::Digit3 => KeyCode::Digit3,
            WKeyCode::Digit4 => KeyCode::Digit4,
            WKeyCode::Digit5 => KeyCode::Digit5,
            WKeyCode::Digit6 => KeyCode::Digit6,
            WKeyCode::Digit7 => KeyCode::Digit7,
            WKeyCode::Digit8 => KeyCode::Digit8,
            WKeyCode::Digit9 => KeyCode::Digit9,

            WKeyCode::F1 => KeyCode::F1,
            WKeyCode::F2 => KeyCode::F2,
            WKeyCode::F3 => KeyCode::F3,
            WKeyCode::F4 => KeyCode::F4,
            WKeyCode::F5 => KeyCode::F5,
            WKeyCode::F6 => KeyCode::F6,
            WKeyCode::F7 => KeyCode::F7,
            WKeyCode::F8 => KeyCode::F8,
            WKeyCode::F9 => KeyCode::F9,
            WKeyCode::F10 => KeyCode::F10,
            WKeyCode::F11 => KeyCode::F11,
            WKeyCode::F12 => KeyCode::F12,

            _ => KeyCode::Unknown,
        }
    }

    #[inline]
    fn map_mouse_button(btn: winit::event::MouseButton) -> MouseButton {
        match btn {
            winit::event::MouseButton::Left => MouseButton::Left,
            winit::event::MouseButton::Right => MouseButton::Right,
            winit::event::MouseButton::Middle => MouseButton::Middle,
            winit::event::MouseButton::Back => MouseButton::Other(4),
            winit::event::MouseButton::Forward => MouseButton::Other(5),
            winit::event::MouseButton::Other(v) => MouseButton::Other(v),
        }
    }

    #[inline]
    fn map_state(s: ElementState) -> KeyState {
        match s {
            ElementState::Pressed => KeyState::Pressed,
            ElementState::Released => KeyState::Released,
        }
    }
}

impl<E: Send + 'static> ApplicationHandler for App<E> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("window create failed");

        self.window = Some(window);

        self.install_window_handles_resource();
        self.emit_ready();
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let _ = self
                    .engine
                    .emit(HostEvent::Window(WindowHostEvent::CloseRequested));
                Self::exit(event_loop);
                return;
            }

            WindowEvent::Resized(PhysicalSize { width, height }) => {
                self.emit_resized(width, height);
            }

            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some((w, h)) = self.window_size() {
                    self.emit_resized(w, h);
                }
            }

            WindowEvent::Focused(focused) => {
                self.emit_focused(focused);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let state = Self::map_state(event.state);

                let repeat = event.repeat;

                let code = match event.physical_key {
                    PhysicalKey::Code(c) => Self::map_key_code(c),
                    PhysicalKey::Unidentified(_) => KeyCode::Unknown,
                };

                let _ = self.engine.emit(HostEvent::Input(InputHostEvent::Key {
                    code,
                    state,
                    repeat,
                }));

                if code == KeyCode::Escape && state == KeyState::Pressed {
                    Self::exit(event_loop);
                    return;
                }

                if let Some(text) = event.text.as_ref() {
                    for ch in text.chars() {
                        let _ = self.engine.emit(HostEvent::Text(TextHostEvent::Char(ch)));
                    }
                }

                let _ = &event.logical_key;
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let _ = self.engine.emit(HostEvent::Input(InputHostEvent::MouseButton {
                    button: Self::map_mouse_button(button),
                    state: Self::map_state(state),
                }));
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 120.0, y * 120.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                let _ = self.engine.emit(HostEvent::Input(InputHostEvent::MouseWheel {
                    dx,
                    dy,
                }));
            }

            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;
                if let Some((px, py)) = self.last_cursor_pos {
                    let _ = self.engine.emit(HostEvent::Input(InputHostEvent::MouseDelta {
                        dx: x - px,
                        dy: y - py,
                    }));
                }
                self.last_cursor_pos = Some((x, y));
                let _ = self.engine.emit(HostEvent::Input(InputHostEvent::MouseMove { x, y }));
            }

            WindowEvent::Ime(ime) => match ime {
                Ime::Commit(text) => {
                    let _ = self.engine.emit(HostEvent::Text(TextHostEvent::ImeCommit(text)));
                }
                Ime::Preedit(text, _) => {
                    let _ = self.engine.emit(HostEvent::Text(TextHostEvent::ImePreedit(text)));
                }
                Ime::Enabled => {}
                Ime::Disabled => {}
            }

            _ => {}
        }

        self.request_redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.engine.step() {
            Ok(_) => self.request_redraw(),
            Err(EngineError::ExitRequested) => Self::exit(event_loop),
            Err(_) => Self::exit(event_loop),
        }
    }
}

pub fn run_winit_app<E: Send + 'static>(engine: Engine<E>) -> EngineResult<()> {
    let event_loop = EventLoop::new().map_err(|e| EngineError::Other(e.to_string()))?;
    let mut app = App::new(engine);

    event_loop
        .run_app(&mut app)
        .map_err(|e| EngineError::Other(e.to_string()))
}
