#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

use abi_stable::std_types::{ROption, RResult, RString};
use newengine_platform_api::{
    NativeWindowBackendV1, NativeWindowHandlesV1, PlatformAppConfigV1,
    PlatformCursorGrabModeV1, PlatformCursorPollV1, PlatformHostApiV1,
    PlatformRuntimeRunFnV1, PlatformSurfaceMetricsV1, PlatformWindowPlacementKindV1,
    PlatformWindowReadyV1,
};
use newengine_plugin_api::{Blob, CapabilityId, HostApiV1, MethodName};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Icon, Window, WindowAttributes, WindowId};

const INPUT_SERVICE_ID: &str = "newengine.input.v1";

struct RuntimeApp {
    plugin_host: HostApiV1,
    host: PlatformHostApiV1,
    config: PlatformAppConfigV1,
    window: Option<Window>,
    last_frame_instant: Option<Instant>,
    last_cursor_pos: Option<(f32, f32)>,
    cursor_captured: bool,
    capture_anchor_px: Option<(f32, f32)>,
    raw_mouse_delta_px: (f32, f32),
    suppress_next_mouse_delta: bool,
    fatal: Option<RString>,
    shutting_down: bool,
}

impl RuntimeApp {
    fn new(plugin_host: HostApiV1, host: PlatformHostApiV1, config: PlatformAppConfigV1) -> Self {
        Self {
            plugin_host,
            host,
            config,
            window: None,
            last_frame_instant: None,
            last_cursor_pos: None,
            cursor_captured: false,
            capture_anchor_px: None,
            raw_mouse_delta_px: (0.0, 0.0),
            suppress_next_mouse_delta: false,
            fatal: None,
            shutting_down: false,
        }
    }

    fn call_host<T>(r: RResult<T, RString>) -> Option<T> {
        match r {
            RResult::ROk(v) => Some(v),
            RResult::RErr(e) => {
                log::error!("platform runtime host callback failed: {}", e);
                None
            }
        }
    }

    fn send_input_json(&self, topic: &'static str, data: serde_json::Value) {
        let payload = serde_json::json!({
            "topic": topic,
            "data": data,
        });

        let Ok(bytes) = serde_json::to_vec(&payload) else {
            return;
        };

        let cap: CapabilityId = RString::from(INPUT_SERVICE_ID);
        let method: MethodName = RString::from("ingest_json");
        let _ = (self.plugin_host.call_service_v1)(cap, method, Blob::from(bytes));
    }

    fn build_window_attributes(
        event_loop: &ActiveEventLoop,
        config: &PlatformAppConfigV1,
    ) -> WindowAttributes {
        let mut attrs = WindowAttributes::default()
            .with_title(config.title.to_string())
            .with_inner_size(PhysicalSize::new(config.width, config.height));

        if let ROption::RSome(icon) = &config.icon {
            if let Ok(wicon) = Icon::from_rgba(icon.rgba.to_vec(), icon.width, icon.height) {
                attrs = attrs.with_window_icon(Some(wicon));
            }
        }

        match config.placement.kind {
            PlatformWindowPlacementKindV1::OsDefault => attrs,
            PlatformWindowPlacementKindV1::Absolute => {
                attrs.with_position(PhysicalPosition::new(config.placement.x, config.placement.y))
            }
            PlatformWindowPlacementKindV1::Centered => {
                let Some(monitor) = event_loop.primary_monitor() else {
                    return attrs;
                };
                let ms = monitor.size();
                let mp = monitor.position();
                let cx = mp.x.saturating_add(((ms.width as i32).saturating_sub(config.width as i32)) / 2);
                let cy = mp.y.saturating_add(((ms.height as i32).saturating_sub(config.height as i32)) / 2);
                attrs.with_position(PhysicalPosition::new(
                    cx.saturating_add(config.placement.x),
                    cy.saturating_add(config.placement.y),
                ))
            }
        }
    }

    fn surface_metrics(&self) -> PlatformSurfaceMetricsV1 {
        match self.window.as_ref() {
            Some(w) => {
                let size = w.inner_size();
                PlatformSurfaceMetricsV1 {
                    width: size.width,
                    height: size.height,
                    pixels_per_point: w.scale_factor() as f32,
                }
            }
            None => PlatformSurfaceMetricsV1::default(),
        }
    }

    fn native_handles(window: &Window) -> NativeWindowHandlesV1 {
        let raw_window = match window.window_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return NativeWindowHandlesV1::default(),
        };
        let raw_display = match window.display_handle() {
            Ok(h) => h.as_raw(),
            Err(_) => return NativeWindowHandlesV1::default(),
        };

        match (raw_window, raw_display) {
            (RawWindowHandle::Win32(w), RawDisplayHandle::Windows(_)) => NativeWindowHandlesV1 {
                backend: NativeWindowBackendV1::Win32,
                window: w.hwnd.get() as usize as u64,
                display: w.hinstance.map(|v| v.get() as usize as u64).unwrap_or(0),
                reserved0: 0,
                reserved1: 0,
            },
            _ => NativeWindowHandlesV1::default(),
        }
    }

    fn apply_cursor_state(window: &Window, poll: PlatformCursorPollV1) {
        if !poll.has_value {
            return;
        }

        window.set_cursor_visible(poll.state.visible);

        use winit::window::CursorGrabMode as WinitGrab;
        let mut desired = match poll.state.grab {
            PlatformCursorGrabModeV1::None => None,
            PlatformCursorGrabModeV1::Confined => Some(WinitGrab::Confined),
            PlatformCursorGrabModeV1::Locked => Some(WinitGrab::Locked),
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
        self.last_cursor_pos = None;
    }

    fn end_capture(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.cursor_captured = false;
        self.raw_mouse_delta_px = (0.0, 0.0);
        self.suppress_next_mouse_delta = true;

        if let Some(p) = self.capture_anchor_px {
            let _ = window.set_cursor_position(PhysicalPosition::new(p.0 as f64, p.1 as f64));
            self.last_cursor_pos = Some(p);
        } else {
            self.last_cursor_pos = None;
        }

        self.capture_anchor_px = None;
    }

    fn frame_dt_seconds(&mut self) -> f32 {
        let now = Instant::now();
        match self.last_frame_instant.replace(now) {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        }
    }

    fn set_fatal(&mut self, message: impl Into<RString>) {
        self.fatal = Some(message.into());
        self.shutting_down = true;
    }

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

    fn map_state_str(s: ElementState) -> &'static str {
        match s {
            ElementState::Pressed => "pressed",
            ElementState::Released => "released",
        }
    }

    fn key_u32_from_physical_key(k: &PhysicalKey) -> u32 {
        match k {
            PhysicalKey::Code(c) => *c as u32,
            PhysicalKey::Unidentified(_) => 0,
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler for RuntimeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Self::build_window_attributes(event_loop, &self.config);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                self.set_fatal(e.to_string());
                event_loop.exit();
                return;
            }
        };

        let ready = PlatformWindowReadyV1 {
            handles: Self::native_handles(&window),
            surface: PlatformSurfaceMetricsV1 {
                width: window.inner_size().width,
                height: window.inner_size().height,
                pixels_per_point: window.scale_factor() as f32,
            },
        };

        self.window = Some(window);

        if Self::call_host((self.host.on_window_ready_v1)(self.host.user_data, ready)).is_none() {
            self.set_fatal("host.on_window_ready_v1 failed");
            event_loop.exit();
            return;
        }

        self.last_frame_instant = Some(Instant::now());
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let _ = (self.host.on_close_requested_v1)(self.host.user_data);
                self.shutting_down = true;
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                let metrics = self.surface_metrics();
                if Self::call_host((self.host.on_window_resized_v1)(self.host.user_data, metrics)).is_none() {
                    self.set_fatal("host.on_window_resized_v1 failed");
                    event_loop.exit();
                    return;
                }
            }
            WindowEvent::Focused(focused) => {
                if !focused && self.cursor_captured {
                    self.end_capture();
                }
                if Self::call_host((self.host.on_window_focused_v1)(self.host.user_data, focused)).is_none() {
                    self.set_fatal("host.on_window_focused_v1 failed");
                    event_loop.exit();
                    return;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = Self::key_u32_from_physical_key(&event.physical_key);
                let state = Self::map_state_str(event.state);
                self.send_input_json(
                    "winit.key",
                    serde_json::json!({
                        "key": key,
                        "scancode": 0u32,
                        "state": state,
                        "repeat": event.repeat,
                    }),
                );

                if let Some(text) = event.text.as_ref() {
                    for ch in text.chars() {
                        self.send_input_json(
                            "winit.text_char",
                            serde_json::json!({
                                "cp": ch as u32,
                            }),
                        );
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.send_input_json(
                    "winit.mouse_button",
                    serde_json::json!({
                        "button": Self::map_mouse_button_u32(button),
                        "state": Self::map_state_str(state),
                    }),
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 120.0, y * 120.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                self.send_input_json(
                    "winit.mouse_wheel",
                    serde_json::json!({ "dx": dx, "dy": dy }),
                );
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;
                if let Some((px, py)) = self.last_cursor_pos {
                    self.send_input_json(
                        "winit.mouse_delta",
                        serde_json::json!({ "dx": x - px, "dy": y - py }),
                    );
                }
                self.last_cursor_pos = Some((x, y));
                self.send_input_json(
                    "winit.mouse_move",
                    serde_json::json!({ "x": x, "y": y }),
                );
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Commit(text) => {
                    self.send_input_json(
                        "winit.ime_commit",
                        serde_json::json!({ "text": text }),
                    );
                }
                Ime::Preedit(text, _) => {
                    self.send_input_json(
                        "winit.ime_preedit",
                        serde_json::json!({ "text": text }),
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
            self.send_input_json(
                "winit.mouse_delta",
                serde_json::json!({
                    "dx": delta.0 as f32,
                    "dy": delta.1 as f32,
                }),
            );
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.shutting_down {
            event_loop.exit();
            return;
        }

        let poll = (self.host.poll_cursor_state_v1)(self.host.user_data);
        if poll.has_value {
            let want_capture = !matches!(poll.state.grab, PlatformCursorGrabModeV1::None);
            if want_capture != self.cursor_captured {
                if want_capture {
                    self.begin_capture();
                } else {
                    self.end_capture();
                }
            }
            if let Some(window) = &self.window {
                Self::apply_cursor_state(window, poll);
            }
        }

        let dt = self.frame_dt_seconds();
        match (self.host.step_v1)(self.host.user_data, dt) {
            RResult::ROk(step) => {
                if step.exit_requested {
                    self.shutting_down = true;
                    event_loop.exit();
                    return;
                }
            }
            RResult::RErr(e) => {
                self.set_fatal(e);
                event_loop.exit();
                return;
            }
        }

        self.request_redraw();
    }
}

#[no_mangle]
pub unsafe extern "C" fn newengine_platform_runtime_run_v1(
    plugin_host: HostApiV1,
    host: PlatformHostApiV1,
    config: PlatformAppConfigV1,
) -> RResult<(), RString> {
    let event_loop = match EventLoop::new() {
        Ok(v) => v,
        Err(e) => return RResult::RErr(RString::from(e.to_string())),
    };

    let mut app = RuntimeApp::new(plugin_host, host, config);

    if let Err(e) = event_loop.run_app(&mut app) {
        return RResult::RErr(RString::from(e.to_string()));
    }

    match app.fatal {
        Some(e) => RResult::RErr(e),
        None => RResult::ROk(()),
    }
}

pub const _: PlatformRuntimeRunFnV1 = newengine_platform_runtime_run_v1;
