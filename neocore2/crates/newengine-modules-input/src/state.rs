use newengine_core::host_events::{HostEvent, InputHostEvent, KeyCode, KeyState, MouseButton, TextHostEvent};

#[inline(always)]
pub fn mouse_bit(btn: MouseButton) -> u32 {
    match btn {
        MouseButton::Left => 1 << 0,
        MouseButton::Right => 1 << 1,
        MouseButton::Middle => 1 << 2,
        MouseButton::Other(v) => {
            let idx = (v as u32).min(28) + 3;
            1 << idx
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct GamepadId(pub u32);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    Start,
    Select,
    Mode,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Other(u16),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftZ,
    RightZ,
    DPadX,
    DPadY,
    Other(u16),
}

#[derive(Debug, Clone)]
pub enum GamepadEvent {
    Connected { id: GamepadId },
    Disconnected { id: GamepadId },
    Button { id: GamepadId, button: GamepadButton, pressed: bool },
    Axis { id: GamepadId, axis: GamepadAxis, value: f32 },
}

#[derive(Clone)]
pub struct InputState {
    pub keys_down: Vec<bool>,
    pub keys_pressed: Vec<bool>,
    pub keys_released: Vec<bool>,

    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub wheel_dx: f32,
    pub wheel_dy: f32,

    pub mouse_down_bits: u32,
    pub mouse_pressed_bits: u32,
    pub mouse_released_bits: u32,

    pub text: Vec<char>,
    pub ime_preedit: String,
    pub ime_commit: String,

    pub gamepad_events: Vec<GamepadEvent>,

    text_cap: usize,
}

impl InputState {
    #[inline]
    pub fn new(key_count: usize, text_cap: usize) -> Self {
        Self {
            keys_down: vec![false; key_count],
            keys_pressed: vec![false; key_count],
            keys_released: vec![false; key_count],

            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            wheel_dx: 0.0,
            wheel_dy: 0.0,

            mouse_down_bits: 0,
            mouse_pressed_bits: 0,
            mouse_released_bits: 0,

            text: Vec::with_capacity(text_cap),
            ime_preedit: String::new(),
            ime_commit: String::new(),

            gamepad_events: Vec::new(),

            text_cap,
        }
    }

    #[inline(always)]
    pub fn begin_frame(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.wheel_dx = 0.0;
        self.wheel_dy = 0.0;

        self.mouse_pressed_bits = 0;
        self.mouse_released_bits = 0;

        for v in self.keys_pressed.iter_mut() {
            *v = false;
        }
        for v in self.keys_released.iter_mut() {
            *v = false;
        }

        self.text.clear();
        self.ime_preedit.clear();
        self.ime_commit.clear();

        self.gamepad_events.clear();
    }

    #[inline(always)]
    pub fn apply(&mut self, ev: &HostEvent, enable_ime: bool) {
        match ev {
            HostEvent::Input(inp) => match *inp {
                InputHostEvent::Key { code, state, repeat } => self.apply_key(code, state, repeat),
                InputHostEvent::MouseMove { x, y } => {
                    self.mouse_x = x;
                    self.mouse_y = y;
                }
                InputHostEvent::MouseDelta { dx, dy } => {
                    self.mouse_dx += dx;
                    self.mouse_dy += dy;
                }
                InputHostEvent::MouseButton { button, state } => self.apply_mouse_button(button, state),
                InputHostEvent::MouseWheel { dx, dy } => {
                    self.wheel_dx += dx;
                    self.wheel_dy += dy;
                }
            },
            HostEvent::Text(t) => match t {
                TextHostEvent::Char(c) => {
                    if self.text.len() < self.text_cap {
                        self.text.push(*c);
                    }
                }
                TextHostEvent::ImePreedit(s) => {
                    if enable_ime {
                        self.ime_preedit.push_str(s);
                    }
                }
                TextHostEvent::ImeCommit(s) => {
                    if enable_ime {
                        self.ime_commit.push_str(s);
                    }
                }
            },
            _ => {}
        }
    }

    #[inline(always)]
    fn apply_key(&mut self, code: KeyCode, state: KeyState, repeat: bool) {
        let idx = code.to_index();
        if idx >= self.keys_down.len() {
            return;
        }

        match state {
            KeyState::Pressed => {
                if !repeat && !self.keys_down[idx] {
                    self.keys_pressed[idx] = true;
                }
                self.keys_down[idx] = true;
            }
            KeyState::Released => {
                if self.keys_down[idx] {
                    self.keys_released[idx] = true;
                }
                self.keys_down[idx] = false;
            }
        }
    }

    #[inline(always)]
    fn apply_mouse_button(&mut self, btn: MouseButton, state: KeyState) {
        let bit = mouse_bit(btn);
        match state {
            KeyState::Pressed => {
                if (self.mouse_down_bits & bit) == 0 {
                    self.mouse_pressed_bits |= bit;
                }
                self.mouse_down_bits |= bit;
            }
            KeyState::Released => {
                if (self.mouse_down_bits & bit) != 0 {
                    self.mouse_released_bits |= bit;
                }
                self.mouse_down_bits &= !bit;
            }
        }
    }
}