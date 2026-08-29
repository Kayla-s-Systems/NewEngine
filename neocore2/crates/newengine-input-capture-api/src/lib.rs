#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameplayInputCapture {
    pub pointer: bool,
    pub keyboard: bool,
    pub block_gameplay_actions: bool,
    pub block_camera_navigation: bool,
    pub block_player_movement: bool,
    pub release_cursor: bool,
    pub pause_simulation: bool,
}

impl GameplayInputCapture {
    #[inline]
    pub const fn none() -> Self {
        Self {
            pointer: false,
            keyboard: false,
            block_gameplay_actions: false,
            block_camera_navigation: false,
            block_player_movement: false,
            release_cursor: false,
            pause_simulation: false,
        }
    }

    #[inline]
    pub const fn modal() -> Self {
        Self {
            pointer: true,
            keyboard: true,
            block_gameplay_actions: true,
            block_camera_navigation: true,
            block_player_movement: true,
            release_cursor: true,
            pause_simulation: false,
        }
    }

    #[inline]
    pub const fn requests_capture(self) -> bool {
        self.pointer
            || self.keyboard
            || self.block_gameplay_actions
            || self.block_camera_navigation
            || self.block_player_movement
            || self.pause_simulation
    }

    #[inline]
    pub const fn blocks_runtime_input(self) -> bool {
        self.block_gameplay_actions || self.block_camera_navigation || self.block_player_movement
    }

    #[inline]
    pub fn merge(&mut self, other: Self) {
        self.pointer |= other.pointer;
        self.keyboard |= other.keyboard;
        self.block_gameplay_actions |= other.block_gameplay_actions;
        self.block_camera_navigation |= other.block_camera_navigation;
        self.block_player_movement |= other.block_player_movement;
        self.release_cursor |= other.release_cursor;
        self.pause_simulation |= other.pause_simulation;
    }
}
