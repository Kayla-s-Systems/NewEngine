mod gateway;
mod input;
mod presentation;
mod state;

use newengine_core::Resources;
use newengine_ui_api::{
    UiInputCaptureState, UiInputCaptureStateManager, UiInputFrame, UI_SURFACE_ENGINE_CONSOLE,
};

use self::state::{ConsoleLineKind, RuntimeConsoleOverlayState};

const CAPTURE_OWNER: &str = "engine.runtime.console-overlay";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConsoleFrameOutcome {
    /// The raw frame was consumed by the console and must not leak into gameplay/editor consumers.
    pub consumed_input: bool,
    /// Console surface/capture state changed and the Debug UI layer should be redrawn.
    pub draw_refresh: bool,
    pub open: bool,
}

pub(crate) fn prepare_frame(
    resources: &mut Resources,
    raw_input: Option<&UiInputFrame>,
    surface_size_px: [u32; 2],
) -> ConsoleFrameOutcome {
    let mut state = resources
        .remove::<RuntimeConsoleOverlayState>()
        .unwrap_or_default();
    let before_revision = state.revision;
    let input_outcome = input::process(&mut state, raw_input);

    if input_outcome.opened || input_outcome.buffer_changed {
        refresh_suggestions(&mut state);
    }
    if let Some(line) = input_outcome.execute_line.as_deref() {
        execute_line(&mut state, line);
        refresh_suggestions(&mut state);
    }

    update_capture(resources, state.open);

    let size_changed = state.last_published_surface_size != surface_size_px;
    // Retain the console before the first user toggle. Mounting and showing a brand-new
    // retained surface on the same key edge used to make first-open depend on provider
    // frame/cache timing. A hidden retained surface makes the first Backquote edge a
    // normal state/visibility update and is retried until engine.ui actually accepts it.
    let publish_required =
        !state.surface_published || state.published_revision != state.revision || size_changed;
    let mut publish_applied = false;
    if publish_required {
        let node = presentation::surface_node(&state, surface_size_px);
        publish_applied = super::ui_gateway_frame::publish_surface_node(&node);
        if publish_applied {
            state.surface_published = true;
            state.published_revision = state.revision;
            state.last_published_surface_size = surface_size_px;
        }
    }
    if input_outcome.closed && state.surface_published {
        let _ = super::ui_gateway_frame::set_surface_visible(UI_SURFACE_ENGINE_CONSOLE, false);
    }

    let draw_refresh = input_outcome.state_changed
        || before_revision != state.revision
        || publish_applied
        || input_outcome.closed;
    let outcome = ConsoleFrameOutcome {
        consumed_input: input_outcome.consumed,
        draw_refresh,
        open: state.open,
    };
    resources.insert(state);
    outcome
}

/// Returns the provider-facing input while the console owns the modal capture.
/// Keyboard/text/gamepad stay consumed by the console state machine, while pointer
/// position/buttons/wheel continue to reach `engine.ui` so the retained output
/// ScrollArea can wheel-scroll and drag its scrollbar without leaking mouse input
/// into gameplay.
pub(crate) fn provider_mouse_input(raw: Option<&UiInputFrame>) -> UiInputFrame {
    let mut out = raw.cloned().unwrap_or_default();
    out.keys_down.clear();
    out.keys_pressed.clear();
    out.keys_released.clear();
    out.text.clear();
    out.ime_preedit.clear();
    out.ime_commit.clear();
    out.text_edit_ops.clear();
    out.gamepad_buttons.clear();
    out.gamepad_buttons_pressed.clear();
    out.gamepad_buttons_released.clear();
    out.gamepad_axes.clear();
    out.gamepad_connected = 0;
    out
}

#[inline]
pub(crate) fn is_open(resources: &Resources) -> bool {
    resources
        .get::<RuntimeConsoleOverlayState>()
        .is_some_and(|state| state.open)
}

fn refresh_suggestions(state: &mut RuntimeConsoleOverlayState) {
    let input = state.buffer.clone();
    match gateway::suggest(&input) {
        Ok(response) => {
            state.suggestions = response;
            state.last_suggest_input = input;
            state.touch();
        }
        Err(error) => {
            state.suggestions = Default::default();
            if state.last_suggest_input != input {
                state.push_line(ConsoleLineKind::Error, error);
            }
            state.last_suggest_input = input;
        }
    }
}

fn execute_line(state: &mut RuntimeConsoleOverlayState, line: &str) {
    state.follow_output_tail();
    state.push_line(ConsoleLineKind::Command, format!("> {line}"));
    match gateway::execute(line) {
        Ok(response) if response.ok => {
            if let Some(output) = response.output.filter(|output| !output.is_empty()) {
                state.push_line(ConsoleLineKind::Output, output);
            }
        }
        Ok(response) => state.push_line(
            ConsoleLineKind::Error,
            response
                .error
                .unwrap_or_else(|| "command failed without an error message".to_owned()),
        ),
        Err(error) => state.push_line(ConsoleLineKind::Error, error),
    }
}

fn update_capture(resources: &mut Resources, open: bool) {
    let mut manager = resources
        .remove::<UiInputCaptureStateManager>()
        .unwrap_or_default();
    if open {
        manager.add_capture(
            CAPTURE_OWNER,
            UiInputCaptureState::exclusive(
                UI_SURFACE_ENGINE_CONSOLE,
                "interactive runtime developer console (non-pausing exclusive input)",
            ),
        );
    } else {
        manager.remove_capture(CAPTURE_OWNER);
    }
    let resolved = manager.resolve_final_capture();
    resources.insert(manager);
    resources.insert(resolved);
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use newengine_ui_api::keys;

    #[test]
    fn console_capture_is_exclusive_but_non_modal() {
        let mut resources = Resources::default();
        update_capture(&mut resources, true);
        let capture = resources
            .get::<UiInputCaptureState>()
            .expect("resolved console capture");
        assert!(capture.camera_navigation_gated);
        assert!(capture.gameplay_movement_gated);
        assert!(!capture.modal, "developer console must not pause the world");
        assert!(capture.requests_capture());
        assert!(capture
            .surfaces
            .iter()
            .any(|surface| surface == UI_SURFACE_ENGINE_CONSOLE));
    }

    #[test]
    fn provider_mouse_input_preserves_pointer_but_consumes_keyboard_text_and_gamepad() {
        let mut raw = UiInputFrame::default();
        raw.keys_down.insert(keys::BACKQUOTE);
        raw.keys_pressed.insert(keys::BACKQUOTE);
        raw.text = "env.inspect".to_owned();
        raw.mouse_pos = Some((320.0, 180.0));
        raw.mouse_delta = (4.0, -2.0);
        raw.mouse_wheel = (0.0, 3.0);
        raw.mouse_down.insert(1);
        raw.mouse_pressed.insert(1);
        raw.gamepad_connected = 1;
        raw.gamepad_buttons.insert("South".to_owned(), 1.0);

        let routed = provider_mouse_input(Some(&raw));
        assert!(routed.keys_down.is_empty());
        assert!(routed.keys_pressed.is_empty());
        assert!(routed.text.is_empty());
        assert!(routed.gamepad_buttons.is_empty());
        assert_eq!(routed.gamepad_connected, 0);
        assert_eq!(routed.mouse_pos, raw.mouse_pos);
        assert_eq!(routed.mouse_delta, raw.mouse_delta);
        assert_eq!(routed.mouse_wheel, raw.mouse_wheel);
        assert_eq!(routed.mouse_down, raw.mouse_down);
        assert_eq!(routed.mouse_pressed, raw.mouse_pressed);
    }
}
