use newengine_core::host_events::{
    HostEvent, InputHostEvent, KeyCode, KeyState, MouseButton, TextHostEvent, WindowHostEvent,
};
use newengine_core::{Engine, EngineError, EngineResult};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    dpi::PhysicalSize,
    event::{ElementState, Ime, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode as WKeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Window placement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinitWindowPlacement {
    /// Let the OS decide.
    OsDefault,
    /// Place the window in the center of the primary monitor.
    /// `offset` allows fine-tuning (e.g. move slightly up).
    Centered { offset: (i32, i32) },
    /// Place the window at an absolute physical position on screen.
    Absolute { x: i32, y: i32 },
}

/// Winit host configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinitAppConfig {
    pub title: String,
    pub size: (u32, u32),
    pub placement: WinitWindowPlacement,
}

impl Default for WinitAppConfig {
    #[inline]
    fn default() -> Self {
        Self {
            title: "NewEngine".to_owned(),
            size: (1280, 720),
            placement: WinitWindowPlacement::Centered { offset: (0, 0) },
        }
    }
}

/// Engine-thread local window handles (not Send/Sync on some platforms).
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct WinitWindowHandles {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

/// Initial window size snapshot taken right after window creation.
/// Vulkan swapchain bootstrap needs it without storing `winit::Window`.
#[derive(Debug, Clone, Copy)]
pub struct WinitWindowInitSize {
    pub width: u32,
    pub height: u32,
}

struct App<E, F>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    engine: Engine<E>,
    after_window: Option<F>,
    config: WinitAppConfig,
    started: bool,
    fatal: Option<EngineError>,

    window: Option<Window>,
    last_cursor_pos: Option<(f32, f32)>,
}

impl<E, F> App<E, F>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    #[inline]
    fn new(engine: Engine<E>, config: WinitAppConfig, after_window: F) -> Self {
        Self {
            engine,
            after_window: Some(after_window),
            config,
            started: false,
            fatal: None,
            window: None,
            last_cursor_pos: None,
        }
    }

    #[inline]
    fn build_window_attributes(
        event_loop: &ActiveEventLoop,
        config: &WinitAppConfig,
    ) -> WindowAttributes {
        let (width, height) = config.size;
        let mut attrs = WindowAttributes::default()
            .with_title(config.title.clone())
            .with_inner_size(PhysicalSize::new(width, height));

        match config.placement {
            WinitWindowPlacement::OsDefault => attrs,

            WinitWindowPlacement::Absolute { x, y } => {
                attrs = attrs.with_position(PhysicalPosition::new(x, y));
                attrs
            }

            WinitWindowPlacement::Centered { offset: (ox, oy) } => {
                let Some(monitor) = event_loop.primary_monitor() else {
                    return attrs;
                };

                let ms = monitor.size();
                let mp = monitor.position();

                let cx =
                    mp.x.saturating_add(((ms.width as i32).saturating_sub(width as i32)) / 2);
                let cy =
                    mp.y.saturating_add(((ms.height as i32).saturating_sub(height as i32)) / 2);

                attrs = attrs.with_position(PhysicalPosition::new(
                    cx.saturating_add(ox),
                    cy.saturating_add(oy),
                ));
                attrs
            }
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

    fn install_window_init_size_resource(&mut self) {
        let Some((width, height)) = self.window_size() else {
            return;
        };
        self.engine
            .resources_mut()
            .insert(WinitWindowInitSize { width, height });
    }

    fn emit_ready(&mut self) {
        let Some((width, height)) = self.window_size() else {
            return;
        };

        let _ = self
            .engine
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Ready { width, height }));
    }

    #[inline]
    fn emit_resized(&mut self, width: u32, height: u32) {
        let _ = self
            .engine
            .emit(HostEvent::Window(WindowHostEvent::Resized { width, height }));
    }

    #[inline]
    fn emit_focused(&mut self, focused: bool) {
        let _ = self
            .engine
            .emit(HostEvent::Window(WindowHostEvent::Focused(focused)));
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

    fn set_fatal_and_exit(&mut self, event_loop: &ActiveEventLoop, e: EngineError) {
        log::error!("winit host fatal: {e}");
        self.fatal = Some(e);
        Self::exit(event_loop);
    }
}

impl<E, F> ApplicationHandler for App<E, F>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Self::build_window_attributes(event_loop, &self.config);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                self.set_fatal_and_exit(event_loop, EngineError::Other(e.to_string()));
                return;
            }
        };

        self.window = Some(window);

        // Insert raw handles + initial size BEFORE registering modules and engine.start()
        self.install_window_handles_resource();
        self.install_window_init_size_resource();

        if let Some(after) = self.after_window.take() {
            if let Err(e) = after(&mut self.engine) {
                self.set_fatal_and_exit(event_loop, e);
                return;
            }
        }

        if !self.started {
            if let Err(e) = self.engine.start() {
                self.set_fatal_and_exit(event_loop, e);
                return;
            }
            self.started = true;
        }

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
                let _ = self
                    .engine
                    .emit(HostEvent::Input(InputHostEvent::MouseButton {
                        button: Self::map_mouse_button(button),
                        state: Self::map_state(state),
                    }));
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 120.0, y * 120.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                let _ = self
                    .engine
                    .emit(HostEvent::Input(InputHostEvent::MouseWheel { dx, dy }));
            }

            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;
                if let Some((px, py)) = self.last_cursor_pos {
                    let _ = self
                        .engine
                        .emit(HostEvent::Input(InputHostEvent::MouseDelta {
                            dx: x - px,
                            dy: y - py,
                        }));
                }
                self.last_cursor_pos = Some((x, y));
                let _ = self
                    .engine
                    .emit(HostEvent::Input(InputHostEvent::MouseMove { x, y }));
            }

            WindowEvent::Ime(ime) => match ime {
                Ime::Commit(text) => {
                    let _ = self
                        .engine
                        .emit(HostEvent::Text(TextHostEvent::ImeCommit(text)));
                }
                Ime::Preedit(text, _) => {
                    let _ = self
                        .engine
                        .emit(HostEvent::Text(TextHostEvent::ImePreedit(text)));
                }
                Ime::Enabled => {}
                Ime::Disabled => {}
            },

            _ => {}
        }

        self.request_redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.fatal.is_some() {
            Self::exit(event_loop);
            return;
        }
        if !self.started {
            self.request_redraw();
            return;
        }

        match self.engine.step() {
            Ok(_) => self.request_redraw(),
            Err(EngineError::ExitRequested) => Self::exit(event_loop),
            Err(e) => {
                log::error!("engine.step failed: {e}");
                Self::exit(event_loop)
            }
        }
    }
}

/// Runs winit host and starts the engine *after* the window is created.
/// `after_window` is called once, right after inserting `WinitWindowHandles` + `WinitWindowInitSize` into Resources.
/// Use it to register modules that require window handles (Vulkan, CEF, etc.).
pub fn run_winit_app<E, F>(engine: Engine<E>, after_window: F) -> EngineResult<()>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    run_winit_app_with_config(engine, WinitAppConfig::default(), after_window)
}

/// Runs winit host with the provided window configuration and starts the engine *after* the window is created.
///
/// `after_window` is called once, right after inserting `WinitWindowHandles` + `WinitWindowInitSize` into Resources.
/// Use it to register modules that require window handles (Vulkan, CEF, etc.).
pub fn run_winit_app_with_config<E, F>(
    engine: Engine<E>,
    config: WinitAppConfig,
    after_window: F,
) -> EngineResult<()>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    let event_loop = EventLoop::new().map_err(|e| EngineError::Other(e.to_string()))?;
    let mut app = App::new(engine, config, after_window);

    event_loop
        .run_app(&mut app)
        .map_err(|e| EngineError::Other(e.to_string()))
}