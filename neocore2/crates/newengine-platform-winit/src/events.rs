/// Typed events injected into the engine from winit.
///
/// Contract: all variants must be `Send + Sync`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WinitExternalEvent {
    WindowReady { width: u32, height: u32 },
    WindowResized { width: u32, height: u32 },
    WindowFocused(bool),

    Key {
        code: KeyCode,
        state: KeyState,
        repeat: bool,
    },

    MouseButton {
        button: MouseButton,
        state: KeyState,
    },

    MouseWheel {
        delta_x: i32,
        delta_y: i32,
    },

    CursorMoved {
        x: f32,
        y: f32,
    },

    CloseRequested,
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

    F1, F2, F3, F4, F5, F6,
    F7, F8, F9, F10, F11, F12,

    Unknown,
}