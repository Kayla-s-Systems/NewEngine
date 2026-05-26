#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputRuntimeSystem {
    RawInput,
    Bindings,
    Actions,
    Contexts,
    Gamepad,
    CameraLook,
    GameplayMovement,
    UiNavigation,
}

impl InputRuntimeSystem {
    pub const ALL: [InputRuntimeSystem; 8] = [
        InputRuntimeSystem::RawInput,
        InputRuntimeSystem::Bindings,
        InputRuntimeSystem::Actions,
        InputRuntimeSystem::Contexts,
        InputRuntimeSystem::Gamepad,
        InputRuntimeSystem::CameraLook,
        InputRuntimeSystem::GameplayMovement,
        InputRuntimeSystem::UiNavigation,
    ];

    #[inline]
    pub const fn id(self) -> &'static str {
        match self {
            Self::RawInput => "engine.input.raw",
            Self::Bindings => "engine.input.bindings",
            Self::Actions => "engine.input.actions",
            Self::Contexts => "engine.input.contexts",
            Self::Gamepad => "engine.input.gamepad",
            Self::CameraLook => "engine.input.camera_look",
            Self::GameplayMovement => "engine.input.gameplay_movement",
            Self::UiNavigation => "engine.input.ui_navigation",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RawInput => "Raw device input",
            Self::Bindings => "Bindings profile",
            Self::Actions => "Semantic action frame",
            Self::Contexts => "Input context/capture stack",
            Self::Gamepad => "Gamepad backend",
            Self::CameraLook => "Camera look controls",
            Self::GameplayMovement => "Gameplay movement controls",
            Self::UiNavigation => "UI navigation controls",
        }
    }

    #[inline]
    pub const fn owner(self) -> &'static str {
        match self {
            Self::RawInput => "newengine.input",
            Self::Bindings => "newengine-input-bindings-runtime",
            Self::Actions => "newengine-input-actions-api",
            Self::Contexts => "newengine-input-contexts-api",
            Self::Gamepad => "newengine.input/gilrs",
            Self::CameraLook => "newengine-camera-runtime",
            Self::GameplayMovement => "newengine-gameplay.player-controller",
            Self::UiNavigation => "engine.ui.node",
        }
    }

    #[inline]
    pub const fn captures_runtime_controls(self) -> bool {
        matches!(self, Self::CameraLook | Self::GameplayMovement)
    }
}

#[derive(Clone, Debug)]
pub struct InputRuntimeSystemState {
    pub system: InputRuntimeSystem,
    pub enabled: bool,
    pub active: bool,
    pub captured: bool,
    pub reason: String,
    pub frame_index: u64,
}

impl InputRuntimeSystemState {
    #[inline]
    pub fn id(&self) -> &'static str { self.system.id() }
    #[inline]
    pub fn label(&self) -> &'static str { self.system.label() }
    #[inline]
    pub fn owner(&self) -> &'static str { self.system.owner() }
}

#[derive(Clone, Debug, Default)]
pub struct InputRuntimeSystemsSnapshot {
    pub frame_index: u64,
    pub systems: Vec<InputRuntimeSystemState>,
}

impl InputRuntimeSystemsSnapshot {
    #[inline]
    pub fn is_enabled(&self, system: InputRuntimeSystem) -> bool {
        self.systems
            .iter()
            .find(|s| s.system == system)
            .map(|s| s.enabled)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_active(&self, system: InputRuntimeSystem) -> bool {
        self.systems
            .iter()
            .find(|s| s.system == system)
            .map(|s| s.active)
            .unwrap_or(false)
    }
}
