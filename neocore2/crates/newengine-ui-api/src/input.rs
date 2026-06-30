// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiTextEditOpKind {
    #[default]
    InsertText,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveStart,
    MoveEnd,
    SelectAll,
    Copy,
    Cut,
    Paste,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiTextEditOp {
    pub kind: UiTextEditOpKind,
    pub text: String,
    pub source: String,
}

impl UiTextEditOp {
    #[inline]
    pub fn new(kind: UiTextEditOpKind, source: impl Into<String>) -> Self {
        Self { kind, text: String::new(), source: source.into() }
    }

    #[inline]
    pub fn with_text(kind: UiTextEditOpKind, text: impl Into<String>, source: impl Into<String>) -> Self {
        Self { kind, text: text.into(), source: source.into() }
    }
}

/// UI input snapshot consumed by engine UI surfaces and runtime overlays.
///
/// This type lives in `newengine-ui-api` so reusable runtime code can exchange
/// input with the UI domain without depending on a concrete UI implementation
/// crate or any provider package.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiInputFrame {
    pub keys_down: std::collections::BTreeSet<u32>,
    pub keys_pressed: std::collections::BTreeSet<u32>,
    pub keys_released: std::collections::BTreeSet<u32>,

    pub mouse_pos: Option<(f32, f32)>,
    pub mouse_delta: (f32, f32),
    pub mouse_wheel: (f32, f32),

    pub mouse_down: std::collections::BTreeSet<u32>,
    pub mouse_pressed: std::collections::BTreeSet<u32>,
    pub mouse_released: std::collections::BTreeSet<u32>,

    pub text: String,
    pub ime_preedit: String,
    pub ime_commit: String,
    pub text_edit_ops: Vec<UiTextEditOp>,

    pub gamepad_buttons: std::collections::BTreeMap<String, f32>,
    pub gamepad_buttons_pressed: std::collections::BTreeSet<String>,
    pub gamepad_buttons_released: std::collections::BTreeSet<String>,

    pub gamepad_axes: std::collections::BTreeMap<String, f32>,
    pub gamepad_connected: usize,
}

/// Tool/UI capture state published through Resources.
///
/// UI surfaces may gate camera navigation or gameplay movement, but they must not
/// stop raw input sampling/listeners. Runtime consumes this generic DTO without
/// knowing which tool or panel requested capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiInputCaptureState {
    pub sampling_alive: bool,
    pub camera_navigation_gated: bool,
    pub gameplay_movement_gated: bool,
    pub modal: bool,
    pub draw_refresh_requested: bool,
    pub reason: String,
    pub surfaces: Vec<String>,
    /// Owner ids that contributed to this resolved capture state.
    ///
    /// This prevents editor shell, asset browser, pause menu, console and future
    /// tools from overwriting one another through a singleton resource.
    pub contributors: Vec<String>,
}

impl Default for UiInputCaptureState {
    fn default() -> Self { Self::none() }
}

/// Owner-keyed UI input capture aggregator.
///
/// Runtime modules must update their own owner contribution instead of writing
/// `UiInputCaptureState` as a destructive singleton. The resolved state remains
/// available as `UiInputCaptureState` for existing render/input consumers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiInputCaptureStateManager {
    pub contributors: BTreeMap<String, UiInputCaptureState>,
}

impl UiInputCaptureStateManager {
    #[inline]
    pub fn add_capture(&mut self, owner: impl Into<String>, mut capture: UiInputCaptureState) {
        let owner = owner.into();
        capture.sampling_alive = true;
        if capture.reason.trim().is_empty() || capture.reason == "none" {
            capture.reason = owner.clone();
        }
        if !capture.contributors.iter().any(|it| it == &owner) {
            capture.contributors.push(owner.clone());
        }
        self.contributors.insert(owner, capture);
    }

    #[inline]
    pub fn remove_capture(&mut self, owner: &str) {
        self.contributors.remove(owner);
    }

    #[inline]
    pub fn resolve_final_capture(&self) -> UiInputCaptureState {
        let mut out = UiInputCaptureState::none();
        let mut reasons = Vec::new();
        for (owner, capture) in &self.contributors {
            out.sampling_alive |= capture.sampling_alive;
            out.camera_navigation_gated |= capture.camera_navigation_gated;
            out.gameplay_movement_gated |= capture.gameplay_movement_gated;
            out.modal |= capture.modal;
            out.draw_refresh_requested |= capture.draw_refresh_requested;
            for surface in &capture.surfaces {
                if !out.surfaces.iter().any(|it| it == surface) {
                    out.surfaces.push(surface.clone());
                }
            }
            if !out.contributors.iter().any(|it| it == owner) {
                out.contributors.push(owner.clone());
            }
            let reason = capture.reason.trim();
            if !reason.is_empty() && reason != "none" {
                reasons.push(format!("{owner}:{reason}"));
            }
        }
        if !reasons.is_empty() {
            out.reason = reasons.join(" + ");
        }
        out
    }
}


impl UiInputCaptureState {
    #[inline]
    pub fn none() -> Self {
        Self {
            sampling_alive: true,
            camera_navigation_gated: false,
            gameplay_movement_gated: false,
            modal: false,
            draw_refresh_requested: false,
            reason: "none".to_owned(),
            surfaces: Vec::new(),
            contributors: Vec::new(),
        }
    }

    #[inline]
    pub fn modal(surface_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            sampling_alive: true,
            camera_navigation_gated: true,
            gameplay_movement_gated: true,
            modal: true,
            draw_refresh_requested: true,
            reason: reason.into(),
            surfaces: vec![surface_id.into()],
            contributors: Vec::new(),
        }
    }

    #[inline]
    pub fn requests_capture(&self) -> bool {
        self.modal || self.camera_navigation_gated || self.gameplay_movement_gated
    }

    #[inline]
    pub fn merged_with_primary_modal(mut self, primary_modal: bool) -> Self {
        self.sampling_alive = true;
        if primary_modal {
            self.modal = true;
            self.camera_navigation_gated = true;
            self.gameplay_movement_gated = true;
            if !self.surfaces.iter().any(|surface| surface == UI_SURFACE_ENGINE_PRIMARY) {
                self.surfaces.push(UI_SURFACE_ENGINE_PRIMARY.to_owned());
            }
            if !self.contributors.iter().any(|owner| owner == "engine.ui.primary") {
                self.contributors.push("engine.ui.primary".to_owned());
            }
            if self.reason == "none" || self.reason.trim().is_empty() {
                self.reason = "primary UI modal capture".to_owned();
            }
        }
        self
    }
}


impl UiInputFrame {
    #[inline]
    pub fn is_key_down(&self, key: u32) -> bool { self.keys_down.contains(&key) }

    #[inline]
    pub fn is_key_pressed(&self, key: u32) -> bool { self.keys_pressed.contains(&key) }

    #[inline]
    pub fn is_key_released(&self, key: u32) -> bool { self.keys_released.contains(&key) }

    #[inline]
    pub fn is_mouse_down(&self, btn: u32) -> bool { self.mouse_down.contains(&btn) }

    #[inline]
    pub fn is_mouse_pressed(&self, btn: u32) -> bool { self.mouse_pressed.contains(&btn) }

    #[inline]
    pub fn is_mouse_released(&self, btn: u32) -> bool { self.mouse_released.contains(&btn) }

    #[inline]
    pub fn has_gamepad_connected(&self) -> bool { self.gamepad_connected > 0 }

    #[inline]
    pub fn has_gamepad_activity(&self) -> bool {
        self.gamepad_buttons.values().any(|v| v.abs() > 0.5)
            || self.gamepad_axes.values().any(|v| v.abs() > 0.05)
            || !self.gamepad_buttons_pressed.is_empty()
            || !self.gamepad_buttons_released.is_empty()
    }

    #[inline]
    pub fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.gamepad_buttons.get(button).copied().unwrap_or(0.0) > 0.5
    }

    #[inline]
    pub fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.gamepad_buttons_pressed.contains(button)
    }

    #[inline]
    pub fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.gamepad_buttons_released.contains(button)
    }

    #[inline]
    pub fn gamepad_axis(&self, axis: &str) -> f32 {
        self.gamepad_axes.get(axis).copied().unwrap_or(0.0)
    }
}

/// Canonical keyboard ids consumed by editor/UI code.
pub mod keys {
    pub use newengine_input_api::key_code::*;
}
