#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_core::render::UiBackdropPostFxParams;
use newengine_input_bindings_api::{InputBindingsProfile, InputDevicePreference};
use newengine_ui::UiInputFrame;
use newengine_ui_api::{
    pause_menu_layout, UiPauseMenuItemTone, UiPauseMenuMessage,
    UiPauseMenuMessageSeverity, UiPauseMenuState,
};
use newengine_ui_menu_runtime::MenuRuntime;
use newengine_ui_navigation_api::{
    MenuFeedbackEvent, MenuFeedbackSeverity, MenuItemTone,
};

use super::input::ViewportInputSnap;

mod document;
mod navigation;
mod presentation;
mod rebind;
mod routes;


const OPEN_SPEED: f32 = 8.5;
const CLOSE_SPEED: f32 = 10.0;

const TARGET_SYSTEM_COMMAND: &str = "SystemCommand";
const TARGET_INPUT_BINDINGS: &str = "InputBindings";

const EVENT_ENGINE_SHUTDOWN_REQUEST: &str = "engine.shutdown.request";
const EVENT_INPUT_DEVICE_NEXT: &str = "engine.input.device_preference.next";
const EVENT_INPUT_DEVICE_PREVIOUS: &str = "engine.input.device_preference.previous";
const EVENT_INPUT_BINDINGS_RESET: &str = "engine.input.bindings.reset";
const EVENT_INPUT_BINDING_REBIND_BEGIN: &str = "engine.input.binding.rebind.begin";

const DYNAMIC_INPUT_DEVICE_PREFERENCE: &str = "input.device_preference";
const DYNAMIC_INPUT_BINDING_LABEL: &str = "input.binding_label";

#[derive(Clone, Debug)]
pub(super) struct PauseMenuFrameResult {
    pub blocks_gameplay: bool,
    pub exit_requested: bool,
    pub state: UiPauseMenuState,
}

#[derive(Clone, Debug)]
struct PendingRebind {
    action_id: String,
    label: String,
}

#[derive(Clone, Debug)]
struct PauseMenuEventFeedback {
    title: String,
    detail: String,
    severity: UiPauseMenuMessageSeverity,
    age_sec: f32,
    ttl_sec: f32,
}

impl PauseMenuEventFeedback {
    #[inline]
    fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiPauseMenuMessageSeverity,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            severity,
            age_sec: 0.0,
            ttl_sec: 2.25,
        }
    }

    #[inline]
    fn from_menu_feedback(feedback: &MenuFeedbackEvent) -> Self {
        Self {
            title: feedback.title.clone(),
            detail: feedback.detail.clone(),
            severity: severity_from_menu(feedback.severity),
            age_sec: 0.0,
            ttl_sec: feedback.ttl_sec,
        }
    }

    #[inline]
    fn to_ui_message(&self) -> UiPauseMenuMessage {
        UiPauseMenuMessage {
            title: self.title.clone(),
            detail: self.detail.clone(),
            severity: self.severity,
            age_sec: self.age_sec,
            ttl_sec: self.ttl_sec,
        }
    }
}

pub(crate) struct RenderPauseMenuRuntimeState {
    open: bool,
    visual_alpha: f32,
    menu: Option<MenuRuntime>,
    document_load_error: Option<String>,
    document_last_attempt_frame: Option<u64>,
    awaiting_rebind: Option<PendingRebind>,
    feedback: Option<PauseMenuEventFeedback>,
    exit_requested: bool,
    profile: InputBindingsProfile,
}

impl RenderPauseMenuRuntimeState {
    #[inline]
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            visual_alpha: 0.0,
            menu: None,
            document_load_error: None,
            document_last_attempt_frame: None,
            awaiting_rebind: None,
            feedback: None,
            exit_requested: false,
            profile: newengine_input_bindings_runtime::input_bindings_profile_snapshot(),
        }
    }

    pub(super) fn update(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        surface_size_px: [u32; 2],
        dt_sec: f32,
        frame_index: u64,
    ) -> PauseMenuFrameResult {
        self.exit_requested = false;
        self.tick_feedback(dt_sec);

        if input.actions.menu_toggle {
            if self.open {
                self.close(frame_index);
            } else if self.ensure_menu_document_loaded(frame_index) {
                self.open(frame_index);
            } else {
                self.open = true;
                self.awaiting_rebind = None;
                self.flash_feedback(
                    "Pause menu unavailable",
                    "Menu document is not available through engine.assets/VFS yet",
                    UiPauseMenuMessageSeverity::Warning,
                );
                audio(AudioFeedbackKind::UiMenuError, frame_index);
            }
        } else if self.open {
            if self.menu.is_none() {
                self.ensure_menu_document_loaded(frame_index);
            }
            if self.awaiting_rebind.is_some() {
                self.process_rebind_capture(surface_input, input, frame_index);
            } else {
                self.process_navigation(surface_input, input, surface_size_px, frame_index);
            }
        }

        self.advance_visual_alpha(dt_sec);
        let visual_visible = self.open || self.visual_alpha > 0.01;
        PauseMenuFrameResult {
            blocks_gameplay: self.open,
            exit_requested: self.exit_requested,
            state: self.build_ui_state(visual_visible),
        }
    }

    #[inline]
    fn open(&mut self, frame_index: u64) {
        self.open = true;
        if let Some(menu) = self.menu.as_mut() {
            menu.reset_to_root();
        }
        self.awaiting_rebind = None;
        self.profile = newengine_input_bindings_runtime::input_bindings_profile_snapshot();
        self.flash_feedback(
            "Pause menu",
            "Game simulation is paused; gameplay input is captured by UI",
            UiPauseMenuMessageSeverity::Info,
        );
        audio(AudioFeedbackKind::UiMenuOpen, frame_index);
    }

    #[inline]
    fn close(&mut self, frame_index: u64) {
        self.open = false;
        if let Some(menu) = self.menu.as_mut() {
            menu.reset_to_root();
        }
        self.awaiting_rebind = None;
        self.flash_feedback("Resume", "Returning to gameplay", UiPauseMenuMessageSeverity::Success);
        audio(AudioFeedbackKind::UiMenuClose, frame_index);
    }

    fn advance_visual_alpha(&mut self, dt_sec: f32) {
        let target = if self.open { 1.0 } else { 0.0 };
        let speed = if self.open { OPEN_SPEED } else { CLOSE_SPEED };
        let step = (dt_sec.max(0.0) * speed).clamp(0.04, 1.0);
        self.visual_alpha += (target - self.visual_alpha) * step;
        if (self.visual_alpha - target).abs() < 0.01 {
            self.visual_alpha = target;
        }
    }

    fn tick_feedback(&mut self, dt_sec: f32) {
        if let Some(feedback) = self.feedback.as_mut() {
            feedback.age_sec += dt_sec.max(0.0);
            if feedback.age_sec >= feedback.ttl_sec {
                self.feedback = None;
            }
        }
    }

    #[inline]
    fn flash_feedback(
        &mut self,
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiPauseMenuMessageSeverity,
    ) {
        self.feedback = Some(PauseMenuEventFeedback::new(title, detail, severity));
    }

    #[inline]
    fn flash_menu_feedback(&mut self, feedback: &MenuFeedbackEvent) {
        self.feedback = Some(PauseMenuEventFeedback::from_menu_feedback(feedback));
    }

    fn ensure_menu_document_loaded(&mut self, frame_index: u64) -> bool {
        if self.menu.is_some() {
            return true;
        }

        let should_attempt = self
            .document_last_attempt_frame
            .map(|last| frame_index.saturating_sub(last) >= 30)
            .unwrap_or(true);
        if !should_attempt {
            return false;
        }

        self.document_last_attempt_frame = Some(frame_index);
        match document::try_load_pause_menu_document() {
            Ok(menu) => {
                log::info!(
                    "engine.pause_menu: declarative MenuDocument loaded through engine.assets/VFS path='{}'",
                    newengine_ui_navigation_api::ENGINE_PAUSE_MENU_ASSET_PATH
                );
                self.document_load_error = None;
                self.menu = Some(menu);
                true
            }
            Err(err) => {
                log::warn!(
                    "engine.pause_menu: MenuDocument unavailable path='{}' err='{}'",
                    newengine_ui_navigation_api::ENGINE_PAUSE_MENU_ASSET_PATH,
                    err
                );
                self.document_load_error = Some(err);
                false
            }
        }
    }

    #[inline]
    pub(super) fn ui_backdrop_postfx(&self) -> UiBackdropPostFxParams {
        let a = ease_out_cubic(self.visual_alpha);
        UiBackdropPostFxParams {
            enabled: self.open || a > 0.01,
            alpha: a,
            dim_opacity: 0.94 * a,
            blur_radius_px: 22.0 * a,
        }
    }
}

fn hovered_item_index(
    mouse_pos: Option<(f32, f32)>,
    surface_size_px: [u32; 2],
    item_count: usize,
    animation_alpha: f32,
) -> Option<usize> {
    pause_menu_layout(surface_size_px, animation_alpha, item_count).hit_item_index(mouse_pos, item_count)
}

#[inline]
fn audio(kind: AudioFeedbackKind, frame_index: u64) {
    crate::audio_gateway::emit_audio_feedback(kind, frame_index);
}

#[inline]
fn ease_out_cubic(v: f32) -> f32 {
    let x = v.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

#[inline]
fn device_preference_label(pref: InputDevicePreference) -> &'static str {
    match pref {
        InputDevicePreference::KeyboardMouse => "Keyboard / Mouse",
        InputDevicePreference::Gamepad => "Gamepad",
        InputDevicePreference::Hybrid => "Hybrid",
    }
}

#[inline]
fn tone_from_menu(tone: MenuItemTone) -> UiPauseMenuItemTone {
    match tone {
        MenuItemTone::Normal => UiPauseMenuItemTone::Normal,
        MenuItemTone::Accent => UiPauseMenuItemTone::Accent,
        MenuItemTone::Danger => UiPauseMenuItemTone::Danger,
        MenuItemTone::Disabled => UiPauseMenuItemTone::Disabled,
    }
}

#[inline]
fn severity_from_menu(severity: MenuFeedbackSeverity) -> UiPauseMenuMessageSeverity {
    match severity {
        MenuFeedbackSeverity::Info => UiPauseMenuMessageSeverity::Info,
        MenuFeedbackSeverity::Success => UiPauseMenuMessageSeverity::Success,
        MenuFeedbackSeverity::Warning => UiPauseMenuMessageSeverity::Warning,
        MenuFeedbackSeverity::Danger => UiPauseMenuMessageSeverity::Danger,
    }
}

#[inline]
fn audio_feedback_from_route(audio_id: &str) -> AudioFeedbackKind {
    match audio_id {
        "ui.menu.open" => AudioFeedbackKind::UiMenuOpen,
        "ui.menu.close" => AudioFeedbackKind::UiMenuClose,
        "ui.menu.navigate" => AudioFeedbackKind::UiMenuNavigate,
        "ui.menu.back" => AudioFeedbackKind::UiMenuBack,
        "ui.menu.rebind" => AudioFeedbackKind::UiMenuRebind,
        "ui.menu.error" => AudioFeedbackKind::UiMenuError,
        "ui.menu.confirm" => AudioFeedbackKind::UiMenuConfirm,
        _ => AudioFeedbackKind::UiMenuConfirm,
    }
}
