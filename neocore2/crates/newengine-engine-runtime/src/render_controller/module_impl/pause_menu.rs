#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_input_bindings::{
    action, InputBinding, InputBindingDevice, InputBindingPhase, InputBindingRegistration,
    InputBindingsProfile, InputDevicePreference,
};
use newengine_ui::UiInputFrame;
use newengine_core::render::UiBackdropPostFxParams;
use newengine_ui_api::{
    pause_menu_layout, UiPauseMenuItem, UiPauseMenuItemTone, UiPauseMenuMessage,
    UiPauseMenuMessageSeverity, UiPauseMenuState, UiPauseMenuTheme,
};

use super::input::ViewportInputSnap;

const ROOT_RESUME: &str = "root.resume";
const ROOT_SETTINGS: &str = "root.settings";
const ROOT_BINDINGS: &str = "root.bindings";
const ROOT_EXIT: &str = "root.exit";

const SETTINGS_DEVICE: &str = "settings.device_preference";
const SETTINGS_RESET_BINDINGS: &str = "settings.reset_bindings";
const SETTINGS_BACK: &str = "settings.back";

const BINDINGS_BACK: &str = "bindings.back";
const OPEN_SPEED: f32 = 8.5;
const CLOSE_SPEED: f32 = 10.0;

const MENU_BINDING_ACTIONS: &[(&str, &str)] = &[
    (action::PLAYER_MOVE_FORWARD, "Move Forward"),
    (action::PLAYER_MOVE_BACK, "Move Back"),
    (action::PLAYER_MOVE_LEFT, "Move Left"),
    (action::PLAYER_MOVE_RIGHT, "Move Right"),
    (action::PLAYER_SPRINT, "Sprint"),
    (action::CAMERA_VIEW_NEXT, "Cycle Camera"),
    (action::CAMERA_VIEW_FIRST_PERSON, "First Person"),
    (action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, "Third Person Follow"),
    (action::CAMERA_VIEW_THIRD_PERSON_AIM, "Third Person Aim"),
    (action::UI_MENU_TOGGLE, "Pause Menu"),
];

#[derive(Clone, Copy)]
struct PauseMenuItemSpec {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    tone: UiPauseMenuItemTone,
}

impl PauseMenuItemSpec {
    #[inline]
    fn item(self) -> UiPauseMenuItem {
        UiPauseMenuItem::new(self.id, self.label)
            .with_detail(self.detail)
            .with_tone(self.tone)
    }
}

const ROOT_ITEMS: &[PauseMenuItemSpec] = &[
    PauseMenuItemSpec { id: ROOT_RESUME, label: "Resume", detail: "Return to the game instantly", tone: UiPauseMenuItemTone::Accent },
    PauseMenuItemSpec { id: ROOT_SETTINGS, label: "Settings", detail: "Tune runtime, input and presentation", tone: UiPauseMenuItemTone::Normal },
    PauseMenuItemSpec { id: ROOT_BINDINGS, label: "Key Bindings", detail: "Remap gameplay and menu actions", tone: UiPauseMenuItemTone::Normal },
    PauseMenuItemSpec { id: ROOT_EXIT, label: "Exit", detail: "Close the game and return to desktop", tone: UiPauseMenuItemTone::Danger },
];

const SETTINGS_ITEMS: &[PauseMenuItemSpec] = &[
    PauseMenuItemSpec { id: SETTINGS_RESET_BINDINGS, label: "Reset Bindings", detail: "Restore the default gameplay binding profile", tone: UiPauseMenuItemTone::Normal },
    PauseMenuItemSpec { id: SETTINGS_BACK, label: "Back", detail: "Return to pause menu", tone: UiPauseMenuItemTone::Normal },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseMenuPage {
    Root,
    Settings,
    Bindings,
}

impl PauseMenuPage {
    #[inline]
    fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Settings => "settings",
            Self::Bindings => "bindings",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PauseMenuFrameResult {
    pub blocks_gameplay: bool,
    pub exit_requested: bool,
    pub state: UiPauseMenuState,
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
    page: PauseMenuPage,
    root_index: usize,
    settings_index: usize,
    bindings_index: usize,
    hovered_index: Option<usize>,
    awaiting_rebind: Option<String>,
    feedback: Option<PauseMenuEventFeedback>,
    exit_requested: bool,
    profile: InputBindingsProfile,
}

impl RenderPauseMenuRuntimeState {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            visual_alpha: 0.0,
            page: PauseMenuPage::Root,
            root_index: 0,
            settings_index: 0,
            bindings_index: 0,
            hovered_index: None,
            awaiting_rebind: None,
            feedback: None,
            exit_requested: false,
            profile: crate::input_bindings_gateway::input_bindings_profile_snapshot(),
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
            } else {
                self.open(frame_index);
            }
        } else if self.open {
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
        self.page = PauseMenuPage::Root;
        self.hovered_index = None;
        self.awaiting_rebind = None;
        self.profile = crate::input_bindings_gateway::input_bindings_profile_snapshot();
        self.flash_feedback("Pause menu", "Game simulation is paused; gameplay input is captured by UI", UiPauseMenuMessageSeverity::Info);
        audio(AudioFeedbackKind::UiMenuOpen, frame_index);
    }

    #[inline]
    fn close(&mut self, frame_index: u64) {
        self.open = false;
        self.page = PauseMenuPage::Root;
        self.hovered_index = None;
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

    fn process_navigation(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        if input.actions.menu_back {
            self.navigate_back(frame_index);
            return;
        }

        let item_count = self.items_len();
        if item_count == 0 {
            return;
        }

        if let Some(input_frame) = surface_input {
            self.hovered_index = hovered_item_index(
                input_frame.mouse_pos,
                surface_size_px,
                item_count,
                ease_out_cubic(self.visual_alpha),
            );
            if let Some(hovered) = self.hovered_index {
                if hovered != self.selected_index() {
                    self.set_selected_index(hovered);
                    audio(AudioFeedbackKind::UiMenuNavigate, frame_index);
                }
            }
            if input_frame.is_mouse_pressed(1) && self.hovered_index.is_some() {
                self.activate_selected(frame_index);
                return;
            }
        } else {
            self.hovered_index = None;
        }

        let nav_y = input.actions.menu_nav[1];
        if nav_y != 0 {
            let dir = if nav_y > 0 { 1 } else { -1 };
            self.move_selection(dir, frame_index);
        }

        match self.page {
            PauseMenuPage::Settings if self.current_item_id().as_deref() == Some(SETTINGS_DEVICE) => {
                if input.actions.menu_nav[0] < 0 {
                    self.cycle_device_preference(-1, frame_index);
                    return;
                }
                if input.actions.menu_nav[0] > 0 {
                    self.cycle_device_preference(1, frame_index);
                    return;
                }
            }
            _ => {}
        }

        if input.actions.menu_accept {
            self.activate_selected(frame_index);
        }
    }

    fn process_rebind_capture(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        frame_index: u64,
    ) {
        if input.actions.menu_back || input.actions.menu_toggle {
            self.awaiting_rebind = None;
            audio(AudioFeedbackKind::UiMenuBack, frame_index);
            return;
        }
        let Some(action_id) = self.awaiting_rebind.clone() else { return; };
        let Some(input_frame) = surface_input else { return; };

        if let Some(&code) = input_frame.keys_pressed.iter().next() {
            if code == newengine_input_bindings::key_code::ESCAPE {
                self.awaiting_rebind = None;
                audio(AudioFeedbackKind::UiMenuBack, frame_index);
                return;
            }
            let registration = InputBindingRegistration {
                binding: InputBinding::keyboard_pressed(action_id.as_str(), code),
                replace_existing_for_action_device: true,
            };
            match crate::input_bindings_gateway::register_input_binding(registration) {
                Ok(profile) => {
                    self.profile = profile;
                    self.flash_feedback("Binding updated", format!("{} now uses the selected keyboard input", binding_label(&action_id)), UiPauseMenuMessageSeverity::Success);
                    audio(AudioFeedbackKind::UiMenuConfirm, frame_index);
                }
                Err(e) => {
                    log::warn!("pause menu: keyboard rebind rejected action='{}' err='{}'", action_id, e);
                    self.flash_feedback("Rebind failed", e, UiPauseMenuMessageSeverity::Danger);
                    audio(AudioFeedbackKind::UiMenuError, frame_index);
                }
            }
            self.awaiting_rebind = None;
            return;
        }

        if let Some(button) = input_frame.gamepad_buttons_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding::gamepad_button_pressed(action_id.as_str(), button.clone()),
                replace_existing_for_action_device: true,
            };
            match crate::input_bindings_gateway::register_input_binding(registration) {
                Ok(profile) => {
                    self.profile = profile;
                    self.flash_feedback("Binding updated", format!("{} now uses the selected gamepad input", binding_label(&action_id)), UiPauseMenuMessageSeverity::Success);
                    audio(AudioFeedbackKind::UiMenuConfirm, frame_index);
                }
                Err(e) => {
                    log::warn!("pause menu: gamepad rebind rejected action='{}' err='{}'", action_id, e);
                    self.flash_feedback("Rebind failed", e, UiPauseMenuMessageSeverity::Danger);
                    audio(AudioFeedbackKind::UiMenuError, frame_index);
                }
            }
            self.awaiting_rebind = None;
            return;
        }

        if let Some(&button) = input_frame.mouse_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding {
                    action: action_id.clone(),
                    device: InputBindingDevice::MouseButton,
                    code: button,
                    name: None,
                    phase: InputBindingPhase::Pressed,
                },
                replace_existing_for_action_device: true,
            };
            match crate::input_bindings_gateway::register_input_binding(registration) {
                Ok(profile) => {
                    self.profile = profile;
                    self.flash_feedback("Binding updated", format!("{} now uses the selected mouse input", binding_label(&action_id)), UiPauseMenuMessageSeverity::Success);
                    audio(AudioFeedbackKind::UiMenuConfirm, frame_index);
                }
                Err(e) => {
                    log::warn!("pause menu: mouse rebind rejected action='{}' err='{}'", action_id, e);
                    self.flash_feedback("Rebind failed", e, UiPauseMenuMessageSeverity::Danger);
                    audio(AudioFeedbackKind::UiMenuError, frame_index);
                }
            }
            self.awaiting_rebind = None;
        }
    }

    fn activate_selected(&mut self, frame_index: u64) {
        audio(AudioFeedbackKind::UiMenuConfirm, frame_index);
        match self.current_item_id().as_deref() {
            Some(ROOT_RESUME) => self.close(frame_index),
            Some(ROOT_EXIT) => {
                self.flash_feedback("Exit requested", "Shutting down through the engine lifecycle", UiPauseMenuMessageSeverity::Danger);
                self.exit_requested = true;
                self.open = false;
            }
            Some(ROOT_SETTINGS) => {
                self.page = PauseMenuPage::Settings;
                self.hovered_index = None;
                self.flash_feedback("Settings", "Runtime options are editable through declarative menu actions", UiPauseMenuMessageSeverity::Info);
            }
            Some(ROOT_BINDINGS) => {
                self.page = PauseMenuPage::Bindings;
                self.hovered_index = None;
                self.flash_feedback("Input bindings", "Each key, button and action is registered through engine.input.bindings", UiPauseMenuMessageSeverity::Info);
            }
            Some(SETTINGS_DEVICE) => self.cycle_device_preference(1, frame_index),
            Some(SETTINGS_RESET_BINDINGS) => {
                match crate::input_bindings_gateway::reset_input_bindings_profile() {
                    Ok(profile) => {
                        self.profile = profile;
                        self.flash_feedback("Bindings reset", "Default keyboard, mouse and gamepad layout restored", UiPauseMenuMessageSeverity::Success);
                    }
                    Err(e) => {
                        log::warn!("pause menu: reset bindings rejected err='{}'", e);
                        self.flash_feedback("Reset failed", e, UiPauseMenuMessageSeverity::Danger);
                    }
                }
            }
            Some(SETTINGS_BACK) | Some(BINDINGS_BACK) => self.navigate_back(frame_index),
            Some(id) if id.starts_with("binding:") => {
                let action_id = id.trim_start_matches("binding:").to_owned();
                self.awaiting_rebind = Some(action_id.clone());
                self.flash_feedback("Listening", format!("Press a key, mouse button or gamepad button for {}", binding_label(&action_id)), UiPauseMenuMessageSeverity::Warning);
                audio(AudioFeedbackKind::UiMenuRebind, frame_index);
            }
            _ => {
                self.flash_feedback("Unavailable", "This menu action has no command route", UiPauseMenuMessageSeverity::Danger);
                audio(AudioFeedbackKind::UiMenuError, frame_index);
            }
        }
    }

    fn navigate_back(&mut self, frame_index: u64) {
        audio(AudioFeedbackKind::UiMenuBack, frame_index);
        match self.page {
            PauseMenuPage::Root => self.close(frame_index),
            PauseMenuPage::Settings | PauseMenuPage::Bindings => {
                self.page = PauseMenuPage::Root;
                self.awaiting_rebind = None;
                self.hovered_index = None;
                self.flash_feedback("Pause menu", "Returned to the main pause page", UiPauseMenuMessageSeverity::Info);
            }
        }
    }

    fn cycle_device_preference(&mut self, delta: i32, frame_index: u64) {
        self.profile.device_preference = match (self.profile.device_preference, delta.signum()) {
            (InputDevicePreference::KeyboardMouse, s) if s < 0 => InputDevicePreference::Hybrid,
            (InputDevicePreference::KeyboardMouse, _) => InputDevicePreference::Gamepad,
            (InputDevicePreference::Gamepad, s) if s < 0 => InputDevicePreference::KeyboardMouse,
            (InputDevicePreference::Gamepad, _) => InputDevicePreference::Hybrid,
            (InputDevicePreference::Hybrid, s) if s < 0 => InputDevicePreference::Gamepad,
            (InputDevicePreference::Hybrid, _) => InputDevicePreference::KeyboardMouse,
        };
        let _ = crate::input_bindings_gateway::save_input_bindings_profile(self.profile.clone());
        self.flash_feedback("Input device", format!("Preference: {}", device_preference_label(self.profile.device_preference)), UiPauseMenuMessageSeverity::Success);
        audio(AudioFeedbackKind::UiMenuNavigate, frame_index);
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

    fn build_ui_state(&self, visual_visible: bool) -> UiPauseMenuState {
        if !visual_visible {
            return UiPauseMenuState::hidden();
        }

        let page_title = match self.page {
            PauseMenuPage::Root => "Pause Menu",
            PauseMenuPage::Settings => "Settings",
            PauseMenuPage::Bindings => "Key Bindings",
        };

        let mut footer_lines = vec![
            "ESC / START - Resume or close pause menu".to_owned(),
            "ARROWS / DPAD - Navigate".to_owned(),
            "ENTER / A / CLICK - Confirm".to_owned(),
            "BACKSPACE / B - Back".to_owned(),
        ];
        if let Some(action_id) = self.awaiting_rebind.as_deref() {
            let label = binding_label(action_id);
            footer_lines.insert(0, format!("Listening for new input: {}", label));
            footer_lines.insert(1, "Press a key, mouse button, or gamepad button".to_owned());
        }

        let a = ease_out_cubic(self.visual_alpha);
        UiPauseMenuState {
            version: 1,
            surface_id: newengine_ui_api::UI_SURFACE_ENGINE_PAUSE_MENU.to_owned(),
            visible: visual_visible,
            paused: self.open,
            page: self.page.as_str().to_owned(),
            title: "PAUSE".to_owned(),
            subtitle: format!("GAME READY / {}", page_title),
            items: self.items(),
            selected_index: self.selected_index(),
            hovered_index: self.hovered_index,
            footer_lines,
            animation_alpha: a,
            backdrop_opacity: 0.94 * a,
            blur_radius_px: 22.0 * a,
            theme: UiPauseMenuTheme::default(),
            message: self.feedback.as_ref().map(PauseMenuEventFeedback::to_ui_message),
        }
    }

    fn items(&self) -> Vec<UiPauseMenuItem> {
        match self.page {
            PauseMenuPage::Root => ROOT_ITEMS.iter().copied().map(PauseMenuItemSpec::item).collect(),
            PauseMenuPage::Settings => {
                let mut items = Vec::with_capacity(SETTINGS_ITEMS.len() + 1);
                items.push(
                    UiPauseMenuItem::new(SETTINGS_DEVICE, "Input Device")
                        .with_value(device_preference_label(self.profile.device_preference))
                        .with_detail("Choose how semantic input actions are resolved")
                        .with_tone(UiPauseMenuItemTone::Accent)
                        .emphasized(true),
                );
                items.extend(SETTINGS_ITEMS.iter().copied().map(PauseMenuItemSpec::item));
                items
            },
            PauseMenuPage::Bindings => {
                let mut out = Vec::with_capacity(MENU_BINDING_ACTIONS.len() + 1);
                for &(action_id, label) in MENU_BINDING_ACTIONS {
                    let mut item = UiPauseMenuItem::new(format!("binding:{}", action_id), label)
                        .with_value(self.profile.primary_binding_label(action_id))
                        .with_detail("Press Enter to rebind this action");
                    if self.awaiting_rebind.as_deref() == Some(action_id) {
                        item = item.with_detail("Press a new key or button now").emphasized(true);
                    }
                    out.push(item);
                }
                out.push(UiPauseMenuItem::new(BINDINGS_BACK, "Back").with_detail("Return to pause menu").with_tone(UiPauseMenuItemTone::Accent));
                out
            }
        }
    }

    #[inline]
    fn items_len(&self) -> usize { self.items().len() }

    #[inline]
    fn selected_index(&self) -> usize {
        match self.page {
            PauseMenuPage::Root => self.root_index,
            PauseMenuPage::Settings => self.settings_index,
            PauseMenuPage::Bindings => self.bindings_index,
        }
    }

    #[inline]
    fn set_selected_index(&mut self, value: usize) {
        match self.page {
            PauseMenuPage::Root => self.root_index = value,
            PauseMenuPage::Settings => self.settings_index = value,
            PauseMenuPage::Bindings => self.bindings_index = value,
        }
    }

    fn move_selection(&mut self, delta: i32, frame_index: u64) {
        let len = self.items_len();
        if len == 0 {
            return;
        }
        let current = self.selected_index() as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        if next != self.selected_index() {
            self.set_selected_index(next);
            audio(AudioFeedbackKind::UiMenuNavigate, frame_index);
        }
    }

    fn current_item_id(&self) -> Option<String> {
        let items = self.items();
        items.get(self.selected_index()).map(|item| item.id.clone())
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
fn binding_label(action_id: &str) -> &'static str {
    MENU_BINDING_ACTIONS
        .iter()
        .find(|(id, _)| *id == action_id)
        .map(|(_, label)| *label)
        .unwrap_or("Binding")
}
