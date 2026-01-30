use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[derive(Debug, Clone)]
pub enum HostEvent {
    Window(WindowHostEvent),
    Input(InputHostEvent),
    Text(TextHostEvent),
}

#[derive(Debug, Clone, Copy)]
pub enum WindowHostEvent {
    /// Window became available (handles are provided via Resources, not events).
    Ready {
        width: u32,
        height: u32,
    },
    Resized {
        width: u32,
        height: u32,
    },
    Focused(bool),
    CloseRequested,
}

#[derive(Debug, Clone, Copy)]
pub enum InputHostEvent {
    Key {
        code: KeyCode,
        state: KeyState,
        repeat: bool,
    },
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseDelta {
        dx: f32,
        dy: f32,
    },
    MouseButton {
        button: MouseButton,
        state: KeyState,
    },
    MouseWheel {
        dx: f32,
        dy: f32,
    },
}

#[derive(Debug, Clone)]
pub enum TextHostEvent {
    Char(char),
    ImePreedit(String),
    ImeCommit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,

    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,

    F1, F2, F3, F4, F5, F6, F7, F8, F9,
    F10, F11, F12,

    Unknown,
}

impl KeyCode {
    #[inline(always)]
    pub const fn to_index(self) -> usize {
        self as usize
    }
}

/// Platform window handles are not Send/Sync on some targets (iOS UIKit).
/// Store them in Resources and access only on the owning thread.
#[derive(Debug, Clone, Copy)]
pub struct WindowHandles {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}