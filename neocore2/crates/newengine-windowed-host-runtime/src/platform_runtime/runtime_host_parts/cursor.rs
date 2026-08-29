use newengine_core::host_events::{CursorState, HostEvent, WindowHostEvent};
use newengine_platform_api::PlatformCursorPollV1;

use super::super::HostPlatformRuntime;
use super::mapping::cursor_poll_from_state;

impl HostPlatformRuntime {
    pub(crate) fn poll_cursor_state(&mut self) -> PlatformCursorPollV1 {
        let mut last: Option<CursorState> = None;

        self.host_events.drain(|ev| {
            if let HostEvent::Window(WindowHostEvent::Cursor(state)) = ev.as_ref() {
                last = Some(*state);
            }
        });

        if let Some(state) = last {
            self.last_platform_cursor = state;
            self.force_cursor_reapply = false;
            return cursor_poll_from_state(state);
        }

        if self.force_cursor_reapply {
            self.force_cursor_reapply = false;
            return cursor_poll_from_state(self.last_platform_cursor);
        }

        // Cursor ownership is state, not an edge. Re-publish the last desired state
        // every platform frame so winit can restore a lost OS grab after focus/UI churn.
        cursor_poll_from_state(self.last_platform_cursor)
    }
}
