#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

use newengine_core::host_events::{HostEvent, WindowHostEvent};
use newengine_core::startup::UiBackend;
use newengine_core::{Engine, EngineError, EngineResult};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::{Window, WindowAttributes, WindowId},
};

use newengine_ui::draw::UiDrawList;
use newengine_ui::{create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind, UiProviderOptions};

use crate::app::config::{WinitAppConfig, WinitWindowPlacement};
use crate::app::input_bridge::{emit_plugin_json, poll_input_frame};
use crate::app::resources::{WinitWindowHandles, WinitWindowInitSize};

pub(crate) struct App<E, F>
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

    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,

    last_frame_instant: Option<Instant>,
    shutting_down: bool,
}

impl<E, F> App<E, F>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    #[inline]
    fn map_ui_backend_to_provider_kind(ui: &UiBackend) -> UiProviderKind {
        match ui {
            UiBackend::Egui => UiProviderKind::Egui,
            UiBackend::Disabled => UiProviderKind::Null,
            UiBackend::Custom(_) => UiProviderKind::Null,
        }
    }

    #[inline]
    pub(crate) fn new(
        engine: Engine<E>,
        config: WinitAppConfig,
        ui_build: Option<Box<dyn UiBuildFn>>,
        after_window: F,
    ) -> Self {
        let kind = Self::map_ui_backend_to_provider_kind(&config.ui_backend);

        if let UiBackend::Custom(name) = &config.ui_backend {
            log::warn!(
                "ui backend '{}' is not supported by this host; falling back to Null",
                name
            );
        }

        let ui = create_provider(UiProviderOptions { kind });

        Self {
            engine,
            after_window: Some(after_window),
            config,
            started: false,
            fatal: None,
            window: None,
            last_cursor_pos: None,
            ui,
            ui_build,
            last_frame_instant: None,
            shutting_down: false,
        }
    }

    #[inline]
    fn build_window_attributes(event_loop: &ActiveEventLoop, config: &WinitAppConfig) -> WindowAttributes {
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

                let cx = mp.x.saturating_add(((ms.width as i32).saturating_sub(width as i32)) / 2);
                let cy = mp.y.saturating_add(((ms.height as i32).saturating_sub(height as i32)) / 2);

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
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window.as_ref().map(|w| {
            let PhysicalSize { width, height } = w.inner_size();
            (width, height)
        })
    }

    #[inline]
    fn emit_resized(&mut self, width: u32, height: u32) {
        self.engine.resources_mut().insert(WinitWindowInitSize { width, height });
        let _ = self.engine.emit(HostEvent::Window(WindowHostEvent::Resized { width, height }));
    }

    fn install_window_handles_resource(&mut self) {
        let Some(w) = &self.window else { return; };

        let window = match w.window_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return,
        };

        let display = match w.display_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return,
        };

        self.engine.resources_mut().insert(WinitWindowHandles { window, display });
    }

    fn install_window_init_size_resource(&mut self) {
        let Some((width, height)) = self.window_size() else { return; };
        self.engine.resources_mut().insert(WinitWindowInitSize { width, height });
    }

    fn emit_ready(&mut self) {
        let Some((width, height)) = self.window_size() else { return; };
        let _ = self
            .engine
            .events()
            .publish(HostEvent::Window(WindowHostEvent::Ready { width, height }));
    }

    #[inline]
    fn emit_focused(&mut self, focused: bool) {
        let _ = self.engine.emit(HostEvent::Window(WindowHostEvent::Focused(focused)));
    }

    #[inline]
    fn frame_dt_seconds(&mut self) -> f32 {
        let now = Instant::now();
        match self.last_frame_instant.replace(now) {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        }
    }

    #[inline]
    fn map_mouse_button_u32(btn: winit::event::MouseButton) -> u32 {
        match btn {
            winit::event::MouseButton::Left => 1,
            winit::event::MouseButton::Right => 2,
            winit::event::MouseButton::Middle => 3,
            winit::event::MouseButton::Back => 4,
            winit::event::MouseButton::Forward => 5,
            winit::event::MouseButton::Other(v) => v as u32,
        }
    }

    #[inline]
    fn map_state_str(s: ElementState) -> &'static str {
        match s {
            ElementState::Pressed => "pressed",
            ElementState::Released => "released",
        }
    }

    #[inline]
    fn key_u32_from_physical_key(k: &PhysicalKey) -> u32 {
        match k {
            PhysicalKey::Code(c) => *c as u32,
            PhysicalKey::Unidentified(_) => 0,
        }
    }

    fn set_fatal_and_exit(&mut self, event_loop: &ActiveEventLoop, e: EngineError) {
        log::error!("winit host fatal: {e}");
        self.fatal = Some(e);
        self.shutdown_and_exit(event_loop);
    }

    fn shutdown_and_exit(&mut self, event_loop: &ActiveEventLoop) {
        if self.shutting_down {
            event_loop.exit();
            return;
        }

        self.shutting_down = true;

        let _ = self.engine.emit(HostEvent::Window(WindowHostEvent::CloseRequested));
        let _ = self.engine.request_exit();

        if let Err(e) = self.engine.shutdown() {
            log::error!("engine.shutdown failed: {e}");
        }

        event_loop.exit();
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
            self.last_frame_instant = Some(Instant::now());
        }

        self.emit_ready();
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // IMPORTANT: No UI backend is allowed to consume platform input directly.
        // All input must flow through the INPUT plugin.

        match event {
            WindowEvent::CloseRequested => {
                self.shutdown_and_exit(event_loop);
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

            // forward-only to input plugin
            WindowEvent::KeyboardInput { event, .. } => {
                let key = Self::key_u32_from_physical_key(&event.physical_key);
                let state = Self::map_state_str(event.state);
                let repeat = event.repeat;

                emit_plugin_json(
                    "winit.key",
                    serde_json::json!({
                        "key": key,
                        "scancode": 0u32,
                        "state": state,
                        "repeat": repeat
                    }),
                );

                if let Some(text) = event.text.as_ref() {
                    for ch in text.chars() {
                        emit_plugin_json(
                            "winit.text_char",
                            serde_json::json!({
                                "cp": ch as u32
                            }),
                        );
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let b = Self::map_mouse_button_u32(button);
                let st = Self::map_state_str(state);

                emit_plugin_json(
                    "winit.mouse_button",
                    serde_json::json!({
                        "button": b,
                        "state": st
                    }),
                );
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 120.0, y * 120.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };

                emit_plugin_json(
                    "winit.mouse_wheel",
                    serde_json::json!({
                        "dx": dx,
                        "dy": dy
                    }),
                );
            }

            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;

                if let Some((px, py)) = self.last_cursor_pos {
                    emit_plugin_json(
                        "winit.mouse_delta",
                        serde_json::json!({
                            "dx": x - px,
                            "dy": y - py
                        }),
                    );
                }

                self.last_cursor_pos = Some((x, y));

                emit_plugin_json(
                    "winit.mouse_move",
                    serde_json::json!({
                        "x": x,
                        "y": y
                    }),
                );
            }

            WindowEvent::Ime(ime) => match ime {
                Ime::Commit(text) => {
                    emit_plugin_json(
                        "winit.ime_commit",
                        serde_json::json!({
                            "text": text
                        }),
                    );
                }
                Ime::Preedit(text, _) => {
                    emit_plugin_json(
                        "winit.ime_preedit",
                        serde_json::json!({
                            "text": text
                        }),
                    );
                }
                Ime::Enabled | Ime::Disabled => {}
            },

            _ => {}
        }

        self.request_redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.fatal.is_some() {
            self.shutdown_and_exit(event_loop);
            return;
        }

        if self.shutting_down {
            event_loop.exit();
            return;
        }

        if !self.started {
            self.request_redraw();
            return;
        }

        let dt = self.frame_dt_seconds();
        let input = poll_input_frame(&self.engine);

        if let (Some(w), Some(build)) = (self.window.as_ref(), self.ui_build.as_deref_mut()) {
            let mut desc = UiFrameDesc::new(dt);
            if let Some(inp) = input {
                desc = desc.with_input(inp);
            }

            let out = self.ui.run_frame(w, desc, build);
            self.engine.resources_mut().insert::<UiDrawList>(out.draw_list);
        }

        match self.engine.step() {
            Ok(_) => self.request_redraw(),
            Err(EngineError::ExitRequested) => self.shutdown_and_exit(event_loop),
            Err(e) => {
                log::error!("engine.step failed: {e}");
                self.shutdown_and_exit(event_loop);
            }
        }
    }
}