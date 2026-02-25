#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

use newengine_core::events::EventSub;
use newengine_core::host_events::{CursorGrabMode, CursorState, HostEvent, WindowHostEvent};
use newengine_core::startup::UiBackend;
use newengine_core::{Engine, EngineError, EngineResult};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, ElementState, Ime, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::{Icon, Window, WindowAttributes, WindowId},
};

use newengine_ui::draw::UiDrawList;
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind, UiProviderOptions,
};

use newengine_ui::UiInputFrame;

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

    host_events: EventSub<HostEvent>,

    cursor_captured: bool,
    capture_anchor_px: Option<(f32, f32)>,
    virtual_cursor_px: Option<(f32, f32)>,
    raw_mouse_delta_px: (f32, f32),
    suppress_next_mouse_delta: bool,

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

        let host_events = engine.events().subscribe::<HostEvent>();

        let ui = create_provider(UiProviderOptions { kind });

        Self {
            engine,
            after_window: Some(after_window),
            config,
            started: false,
            fatal: None,
            window: None,
            last_cursor_pos: None,

            host_events,

            cursor_captured: false,
            capture_anchor_px: None,
            virtual_cursor_px: None,
            raw_mouse_delta_px: (0.0, 0.0),
            suppress_next_mouse_delta: false,
            ui,
            ui_build,
            last_frame_instant: None,
            shutting_down: false,
        }
    }

    #[inline]
    fn apply_cursor_state(window: &Window, state: CursorState) {
        window.set_cursor_visible(state.visible);

        use winit::window::CursorGrabMode as WinitGrab;

        let mut desired = match state.grab {
            CursorGrabMode::None => None,
            CursorGrabMode::Confined => Some(WinitGrab::Confined),
            CursorGrabMode::Locked => Some(WinitGrab::Locked),
        };

        while let Some(mode) = desired.take() {
            if window.set_cursor_grab(mode).is_ok() {
                return;
            }

            desired = match mode {
                WinitGrab::Locked => Some(WinitGrab::Confined),
                WinitGrab::Confined => None,
                WinitGrab::None => None,
            };
        }

        let _ = window.set_cursor_grab(WinitGrab::None);
    }

    #[inline]
    fn begin_capture(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.cursor_captured = true;
        self.raw_mouse_delta_px = (0.0, 0.0);
        self.suppress_next_mouse_delta = true;

        let anchor = self
            .last_cursor_pos
            .or_else(|| {
                let s = window.inner_size();
                Some((s.width as f32 * 0.5, s.height as f32 * 0.5))
            })
            .unwrap_or((0.0, 0.0));

        self.capture_anchor_px = Some(anchor);
        self.virtual_cursor_px = Some(anchor);

        Self::apply_cursor_state(window, CursorState::captured_locked());
        self.last_cursor_pos = None;
    }

    #[inline]
    fn end_capture(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.cursor_captured = false;
        self.raw_mouse_delta_px = (0.0, 0.0);
        self.suppress_next_mouse_delta = true;

        Self::apply_cursor_state(window, CursorState::released());

        if let Some(p) = self.capture_anchor_px {
            let _ = window.set_cursor_position(PhysicalPosition::new(p.0 as f64, p.1 as f64));
            self.last_cursor_pos = Some(p);
        } else {
            self.last_cursor_pos = None;
        }

        self.capture_anchor_px = None;
        self.virtual_cursor_px = None;
    }

    #[inline]
    fn drain_cursor_requests(&mut self) {
        let mut last: Option<CursorState> = None;
        self.host_events.drain(|ev| {
            if let HostEvent::Window(WindowHostEvent::Cursor(s)) = ev.as_ref() {
                last = Some(*s);
            }
        });

        let Some(state) = last else {
            return;
        };

        let want_capture = state.grab != CursorGrabMode::None;
        if want_capture != self.cursor_captured {
            if want_capture {
                self.begin_capture();
            } else {
                self.end_capture();
            }
            return;
        }

        if let Some(window) = self.window.as_ref() {
            Self::apply_cursor_state(window, state);
        }
    }

    #[inline]
    fn patch_input_for_capture(&mut self, input: &mut UiInputFrame) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let mut dx = self.raw_mouse_delta_px.0;
        let mut dy = self.raw_mouse_delta_px.1;
        self.raw_mouse_delta_px = (0.0, 0.0);

        if self.suppress_next_mouse_delta {
            dx = 0.0;
            dy = 0.0;
            self.suppress_next_mouse_delta = false;
        }

        input.mouse_delta = (dx, dy);

        let s = window.inner_size();
        let w = s.width.max(1) as f32;
        let h = s.height.max(1) as f32;

        let anchor = self
            .capture_anchor_px
            .unwrap_or((w * 0.5, h * 0.5));

        let mut pos = self.virtual_cursor_px.unwrap_or(anchor);
        pos.0 += dx;
        pos.1 += dy;

        let margin = 2.0;
        if pos.0 < margin || pos.0 > (w - margin) || pos.1 < margin || pos.1 > (h - margin) {
            pos = anchor;
            input.mouse_delta = (0.0, 0.0);
            self.suppress_next_mouse_delta = true;
        }

        self.virtual_cursor_px = Some(pos);
        input.mouse_pos = Some(pos);
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

        // Install window icon (if provided).
        if let Some(icon) = config.icon.as_ref() {
            if let Ok(wicon) = Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height) {
                attrs = attrs.with_window_icon(Some(wicon));
            } else {
                log::warn!("invalid winit icon payload (rgba/size mismatch?)");
            }
        }

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
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window.as_ref().map(|w| {
            let PhysicalSize { width, height } = w.inner_size();
            (width, height)
        })
    }

    #[inline]
    fn emit_resized(&mut self, width: u32, height: u32) {
        self.engine
            .resources_mut()
            .insert(WinitWindowInitSize { width, height });
        let _ = self
            .engine
            .emit(HostEvent::Window(WindowHostEvent::Resized {
                width,
                height,
            }));
    }

    fn install_window_handles_resource(&mut self) {
        let Some(w) = &self.window else {
            return;
        };

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
    fn emit_focused(&mut self, focused: bool) {
        let _ = self
            .engine
            .emit(HostEvent::Window(WindowHostEvent::Focused(focused)));
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

        let _ = self
            .engine
            .emit(HostEvent::Window(WindowHostEvent::CloseRequested));
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

                // Best-effort safety: don't keep the cursor grabbed across focus loss.
                if !focused {
                    if self.cursor_captured {
                        self.end_capture();
                    }
                }
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

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if !self.cursor_captured {
            return;
        }

        if let DeviceEvent::MouseMotion { delta } = event {
            self.raw_mouse_delta_px.0 += delta.0 as f32;
            self.raw_mouse_delta_px.1 += delta.1 as f32;
        }
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

        // Apply cursor requests produced by engine modules on the previous frame.
        self.drain_cursor_requests();

        let dt = self.frame_dt_seconds();
        let mut input = poll_input_frame(&self.engine);

        // When the cursor is captured, prefer raw device motion and keep the UI pointer stable.
        if self.cursor_captured {
            if input.is_none() {
                input = Some(UiInputFrame::default());
            }
            if let Some(ref mut frame) = input {
                self.patch_input_for_capture(frame);
            }
        } else {
            // Keep anchor in sync with the real cursor for a clean capture edge.
            if let Some(ref f) = input {
                if let Some(pos) = f.mouse_pos {
                    self.last_cursor_pos = Some(pos);
                }
            }
            self.raw_mouse_delta_px = (0.0, 0.0);
        }

        if let (Some(w), Some(build)) = (self.window.as_ref(), self.ui_build.as_deref_mut()) {
            let mut desc = UiFrameDesc::new(dt);
            if let Some(inp) = input {
                desc = desc.with_input(inp);
            }

            let out = self.ui.run_frame(w, desc, build);
            self.engine
                .resources_mut()
                .insert::<UiDrawList>(out.draw_list);
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
