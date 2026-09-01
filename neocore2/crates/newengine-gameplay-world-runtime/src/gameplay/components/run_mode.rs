#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameRunMode {
    #[default]
    Staging,
    Simulate,
    Play,
}

impl GameRunMode {
    #[inline]
    pub const fn is_runtime(self) -> bool {
        matches!(self, Self::Simulate | Self::Play)
    }

    #[inline]
    pub const fn runs_physics(self) -> bool {
        self.is_runtime()
    }

    #[inline]
    pub const fn wants_direct_player_control(self) -> bool {
        matches!(self, Self::Play)
    }
}

// Physics components are owned by `newengine-physics-contracts`.
// GameFirst runtime may re-export them for callers, but gameplay code must not
// define its own collision model or store backend-native handles.
