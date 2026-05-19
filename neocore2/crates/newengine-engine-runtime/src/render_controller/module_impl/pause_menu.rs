#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_core::render::UiBackdropPostFxParams;
use newengine_input_bindings_api::{
    InputBinding, InputBindingDevice, InputBindingPhase, InputBindingRegistration,
    InputBindingsProfile, InputDevicePreference,
};
use newengine_ui::UiInputFrame;
use newengine_ui_api::{
    pause_menu_layout, UiPauseMenuItem, UiPauseMenuItemTone, UiPauseMenuMessage,
    UiPauseMenuMessageSeverity, UiPauseMenuState, UiPauseMenuTheme,
};
use newengine_ui_menu_runtime::{
    MenuHitTestState, MenuRouteDispatch, MenuRuntime, MenuRuntimeInput, MenuRuntimeOutput,
};
use newengine_ui_navigation_api::{
    MenuActionRoute, MenuDocument, MenuFeedbackEvent, MenuFeedbackSeverity, MenuItem, MenuItemTone,
};

use super::input::ViewportInputSnap;

const PAUSE_MENU_DOCUMENT_JSON: &str =
    include_str!("../../../../../assets/ui/menus/engine.pause_menu.menu.json");

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
    menu: MenuRuntime,
    awaiting_rebind: Option<PendingRebind>,
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
            menu: load_pause_menu_document(),
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
        self.menu.reset_to_root();
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
        self.menu.reset_to_root();
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

    fn process_navigation(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        let item_count = self.menu.current_items().len();
        if item_count == 0 {
            return;
        }

        let hit_test = surface_input.map(|input_frame| MenuHitTestState {
            hovered_index: hovered_item_index(
                input_frame.mouse_pos,
                surface_size_px,
                item_count,
                ease_out_cubic(self.visual_alpha),
            ),
            pointer_primary_pressed: input_frame.is_mouse_pressed(1),
        });

        let output = self.menu.handle_input(MenuRuntimeInput {
            nav_x: input.actions.menu_nav[0],
            nav_y: input.actions.menu_nav[1],
            accept: input.actions.menu_accept,
            back: input.actions.menu_back,
            hit_test,
        });

        self.apply_menu_runtime_output(output, frame_index);
    }

    fn apply_menu_runtime_output(&mut self, output: MenuRuntimeOutput, frame_index: u64) {
        if output.selection_changed {
            audio(AudioFeedbackKind::UiMenuNavigate, frame_index);
        }

        for feedback in &output.feedback {
            self.flash_menu_feedback(feedback);
        }

        for dispatch in output.route_dispatches {
            self.dispatch_menu_route(dispatch, frame_index);
        }

        if output.close_requested {
            self.open = false;
            self.awaiting_rebind = None;
            if self.feedback.is_none() {
                self.flash_feedback("Resume", "Returning to gameplay", UiPauseMenuMessageSeverity::Success);
            }
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
        let Some(pending) = self.awaiting_rebind.clone() else { return; };
        let Some(input_frame) = surface_input else { return; };

        if let Some(&code) = input_frame.keys_pressed.iter().next() {
            if code == newengine_input_api::key_code::ESCAPE {
                self.awaiting_rebind = None;
                audio(AudioFeedbackKind::UiMenuBack, frame_index);
                return;
            }
            let registration = InputBindingRegistration {
                binding: InputBinding::keyboard_pressed(pending.action_id.as_str(), code),
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "keyboard", frame_index);
            return;
        }

        if let Some(button) = input_frame.gamepad_buttons_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding::gamepad_button_pressed(pending.action_id.as_str(), button.clone()),
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "gamepad", frame_index);
            return;
        }

        if let Some(&button) = input_frame.mouse_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding {
                    action: pending.action_id.clone(),
                    device: InputBindingDevice::MouseButton,
                    code: button,
                    name: None,
                    phase: InputBindingPhase::Pressed,
                },
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "mouse", frame_index);
        }
    }

    fn apply_rebind_registration(
        &mut self,
        registration: InputBindingRegistration,
        pending: &PendingRebind,
        device_label: &str,
        frame_index: u64,
    ) {
        match newengine_input_bindings_runtime::register_input_binding(registration) {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Binding updated",
                    format!("{} now uses the selected {} input", pending.label, device_label),
                    UiPauseMenuMessageSeverity::Success,
                );
                audio(AudioFeedbackKind::UiMenuConfirm, frame_index);
            }
            Err(e) => {
                log::warn!(
                    "pause menu command router: rebind rejected action='{}' err='{}'",
                    pending.action_id,
                    e
                );
                self.flash_feedback("Rebind failed", e, UiPauseMenuMessageSeverity::Danger);
                audio(AudioFeedbackKind::UiMenuError, frame_index);
            }
        }
        self.awaiting_rebind = None;
    }

    fn dispatch_menu_route(&mut self, dispatch: MenuRouteDispatch, frame_index: u64) {
        if let Some(audio_id) = dispatch.route.audio.as_deref() {
            audio(audio_feedback_from_route(audio_id), frame_index);
        }

        let MenuRouteDispatch { route, source_label, .. } = dispatch;
        match (route.target.as_str(), route.event.as_str()) {
            (TARGET_SYSTEM_COMMAND, EVENT_ENGINE_SHUTDOWN_REQUEST) => {
                self.exit_requested = true;
                self.open = false;
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_DEVICE_NEXT) => {
                self.cycle_device_preference(1);
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_DEVICE_PREVIOUS) => {
                self.cycle_device_preference(-1);
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_BINDINGS_RESET) => {
                self.reset_bindings_profile();
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_BINDING_REBIND_BEGIN) => {
                self.begin_binding_rebind(&route, source_label);
            }
            (_, _) => {
                if route.target != "MenuRuntime" {
                    log::warn!(
                        "pause menu command router: unsupported route target='{}' event='{}' id='{}'",
                        route.target,
                        route.event,
                        route.id
                    );
                    self.flash_feedback(
                        "Unavailable",
                        "This menu action has no command route",
                        UiPauseMenuMessageSeverity::Danger,
                    );
                    audio(AudioFeedbackKind::UiMenuError, frame_index);
                }
            }
        }
    }

    fn begin_binding_rebind(&mut self, route: &MenuActionRoute, source_label: Option<String>) {
        let Some(action_id) = route.payload_str("action_id") else {
            self.flash_feedback(
                "Unavailable",
                "Binding route has no action_id payload",
                UiPauseMenuMessageSeverity::Danger,
            );
            return;
        };
        let label = source_label.unwrap_or_else(|| action_id.to_owned());
        self.awaiting_rebind = Some(PendingRebind {
            action_id: action_id.to_owned(),
            label: label.clone(),
        });
        self.flash_feedback(
            "Listening",
            format!("Press a key, mouse button or gamepad button for {}", label),
            UiPauseMenuMessageSeverity::Warning,
        );
    }

    fn reset_bindings_profile(&mut self) {
        match newengine_input_bindings_runtime::reset_input_bindings_profile() {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Bindings reset",
                    "Default keyboard, mouse and gamepad layout restored",
                    UiPauseMenuMessageSeverity::Success,
                );
            }
            Err(e) => {
                log::warn!("pause menu command router: reset bindings rejected err='{}'", e);
                self.flash_feedback("Reset failed", e, UiPauseMenuMessageSeverity::Danger);
            }
        }
    }

    fn cycle_device_preference(&mut self, delta: i32) {
        self.profile.device_preference = match (self.profile.device_preference, delta.signum()) {
            (InputDevicePreference::KeyboardMouse, s) if s < 0 => InputDevicePreference::Hybrid,
            (InputDevicePreference::KeyboardMouse, _) => InputDevicePreference::Gamepad,
            (InputDevicePreference::Gamepad, s) if s < 0 => InputDevicePreference::KeyboardMouse,
            (InputDevicePreference::Gamepad, _) => InputDevicePreference::Hybrid,
            (InputDevicePreference::Hybrid, s) if s < 0 => InputDevicePreference::Gamepad,
            (InputDevicePreference::Hybrid, _) => InputDevicePreference::KeyboardMouse,
        };
        match newengine_input_bindings_runtime::save_input_bindings_profile(self.profile.clone()) {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Input device",
                    format!("Preference: {}", device_preference_label(self.profile.device_preference)),
                    UiPauseMenuMessageSeverity::Success,
                );
            }
            Err(e) => {
                log::warn!("pause menu command router: device preference save failed err='{}'", e);
                self.flash_feedback("Save failed", e, UiPauseMenuMessageSeverity::Danger);
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

    fn build_ui_state(&self, visual_visible: bool) -> UiPauseMenuState {
        if !visual_visible {
            return UiPauseMenuState::hidden();
        }

        let document = self.menu.document();
        let current_page = self.menu.current_page();
        let page_title = current_page
            .map(|page| page.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or("Pause Menu");
        let page_subtitle = current_page
            .map(|page| page.subtitle.as_str())
            .filter(|subtitle| !subtitle.is_empty())
            .unwrap_or_else(|| document.subtitle.as_str());

        let mut footer_lines = current_page
            .and_then(|page| (!page.footer_lines.is_empty()).then(|| page.footer_lines.clone()))
            .unwrap_or_else(|| document.footer_lines.clone());
        if footer_lines.is_empty() {
            footer_lines = vec![
                "ESC / START - Resume or close pause menu".to_owned(),
                "ARROWS / DPAD - Navigate".to_owned(),
                "ENTER / A / CLICK - Confirm".to_owned(),
                "BACKSPACE / B - Back".to_owned(),
            ];
        }
        if let Some(pending) = self.awaiting_rebind.as_ref() {
            footer_lines.insert(0, format!("Listening for new input: {}", pending.label));
            footer_lines.insert(1, "Press a key, mouse button, or gamepad button".to_owned());
        }

        let a = ease_out_cubic(self.visual_alpha);
        UiPauseMenuState {
            version: 1,
            surface_id: document.surface_id.clone(),
            visible: visual_visible,
            paused: self.open,
            page: self.menu.current_page_id().to_owned(),
            title: if document.title.is_empty() { "PAUSE".to_owned() } else { document.title.clone() },
            subtitle: if page_subtitle.is_empty() {
                page_title.to_owned()
            } else {
                format!("{} / {}", page_subtitle, page_title)
            },
            items: self.items(),
            selected_index: self.menu.selected_index(),
            hovered_index: self.menu.hovered_index(),
            footer_lines,
            animation_alpha: a,
            backdrop_opacity: 0.94 * a,
            blur_radius_px: 22.0 * a,
            theme: UiPauseMenuTheme::default(),
            message: self.feedback.as_ref().map(PauseMenuEventFeedback::to_ui_message),
        }
    }

    fn items(&self) -> Vec<UiPauseMenuItem> {
        self.menu
            .current_items()
            .iter()
            .map(|item| self.item_to_ui(item))
            .collect()
    }

    fn item_to_ui(&self, item: &MenuItem) -> UiPauseMenuItem {
        let mut out = UiPauseMenuItem::new(item.id.clone(), item.label.clone())
            .emphasized(item.emphasized)
            .with_tone(tone_from_menu(item.tone));

        if let Some(value) = self.item_value(item) {
            out = out.with_value(value);
        }
        if let Some(detail) = self.item_detail(item) {
            out = out.with_detail(detail);
        }
        out
    }

    fn item_value(&self, item: &MenuItem) -> Option<String> {
        match item.dynamic_value.as_deref() {
            Some(DYNAMIC_INPUT_DEVICE_PREFERENCE) => {
                Some(device_preference_label(self.profile.device_preference).to_owned())
            }
            Some(DYNAMIC_INPUT_BINDING_LABEL) => item
                .action
                .as_ref()
                .and_then(|route| route.payload_str("action_id"))
                .map(|action_id| self.profile.primary_binding_label(action_id)),
            _ => item.value.clone(),
        }
    }

    fn item_detail(&self, item: &MenuItem) -> Option<String> {
        let awaiting_action = self.awaiting_rebind.as_ref().map(|pending| pending.action_id.as_str());
        let item_action = item
            .action
            .as_ref()
            .and_then(|route| route.payload_str("action_id"));
        if awaiting_action.is_some() && awaiting_action == item_action {
            Some("Press a new key or button now".to_owned())
        } else {
            item.detail.clone()
        }
    }
}


fn load_pause_menu_document() -> MenuRuntime {
    let document = MenuDocument::from_json_str(PAUSE_MENU_DOCUMENT_JSON)
        .unwrap_or_else(|e| panic!("invalid engine.pause_menu MenuDocument asset: {e}"));
    MenuRuntime::new(document)
        .unwrap_or_else(|e| panic!("invalid engine.pause_menu MenuDocument contract: {e}"))
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
