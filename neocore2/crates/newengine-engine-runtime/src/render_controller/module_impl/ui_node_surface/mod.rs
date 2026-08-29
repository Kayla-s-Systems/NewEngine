#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_core::render::UiBackdropPostFxParams;
use newengine_input_bindings_api::{InputBindingsProfile, InputDevicePreference};
use newengine_ui_api::UiInputFrame;
use newengine_ui_api::{
    ui_surface_node_layout, UiNodeMessage, UiNodeMessageSeverity, UiNodeTone, UiSurfaceAnchor,
    UiSurfaceNode, UiSurfaceStyle,
};
use newengine_ui_navigation_api::UiNodeNavigationRuntime;
use newengine_ui_navigation_api::{
    UiNodeFeedbackEvent, UiNodeFeedbackSeverity, UiNodeNavigationTone,
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
pub(super) struct UiNodeSurfaceFrameResult {
    pub blocks_gameplay: bool,
    pub exit_requested: bool,
    pub state: UiSurfaceNode,
}

#[derive(Clone, Debug)]
struct PendingRebind {
    action_id: String,
    label: String,
}

#[derive(Clone, Debug)]
struct UiNodeSurfaceEventFeedback {
    title: String,
    detail: String,
    severity: UiNodeMessageSeverity,
    age_sec: f32,
    ttl_sec: f32,
}

impl UiNodeSurfaceEventFeedback {
    #[inline]
    fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiNodeMessageSeverity,
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
    fn from_navigation_feedback(feedback: &UiNodeFeedbackEvent) -> Self {
        Self {
            title: feedback.title.clone(),
            detail: feedback.detail.clone(),
            severity: severity_from_navigation(feedback.severity),
            age_sec: 0.0,
            ttl_sec: feedback.ttl_sec,
        }
    }

    #[inline]
    fn to_ui_message(&self) -> UiNodeMessage {
        UiNodeMessage {
            title: self.title.clone(),
            detail: self.detail.clone(),
            severity: self.severity,
            age_sec: self.age_sec,
            ttl_sec: self.ttl_sec,
        }
    }
}

pub(crate) struct RenderUiNodeSurfaceState {
    open: bool,
    visual_alpha: f32,
    navigation: Option<UiNodeNavigationRuntime>,
    document_load_error: Option<String>,
    document_last_attempt_frame: Option<u64>,
    awaiting_rebind: Option<PendingRebind>,
    feedback: Option<UiNodeSurfaceEventFeedback>,
    exit_requested: bool,
    profile: InputBindingsProfile,
}

impl RenderUiNodeSurfaceState {
    #[inline]
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            visual_alpha: 0.0,
            navigation: None,
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
    ) -> UiNodeSurfaceFrameResult {
        self.exit_requested = false;
        self.tick_feedback(dt_sec);

        if !crate::runtime_policy::render_runtime_policy().primary_ui_enabled {
            self.open = false;
            self.awaiting_rebind = None;
            self.advance_visual_alpha(dt_sec);
            return UiNodeSurfaceFrameResult {
                blocks_gameplay: false,
                exit_requested: false,
                state: self.build_ui_state(false),
            };
        }

        if input.actions.ui_toggle {
            if self.open {
                self.close(frame_index);
            } else if self.ensure_navigation_document_loaded(frame_index) {
                self.open(frame_index);
            } else {
                self.open = true;
                self.awaiting_rebind = None;
                self.flash_feedback(
                    "UI surface unavailable",
                    "UI document is not available through engine.assets/VFS yet",
                    UiNodeMessageSeverity::Warning,
                );
                audio(AudioFeedbackKind::UiError, frame_index);
            }
        } else if self.open {
            if self.navigation.is_none() {
                self.ensure_navigation_document_loaded(frame_index);
            }
            if self.awaiting_rebind.is_some() {
                self.process_rebind_capture(surface_input, input, frame_index);
            } else {
                self.process_navigation(surface_input, input, surface_size_px, frame_index);
            }
        }

        self.advance_visual_alpha(dt_sec);
        // Retained provider state must clear immediately on close. The backdrop can
        // still fade through ui_backdrop_postfx(), but the UiSurfaceNode visibility
        // is authoritative and must not stay true during the close animation.
        let visual_visible = self.open;
        UiNodeSurfaceFrameResult {
            // Open UI is always an input-owning surface. The camera/input listener
            // remains alive, but navigation/gameplay are gated even when the
            // declarative .neui document is temporarily unavailable. Otherwise the
            // user sees a modal warning while the camera continues to move behind it.
            blocks_gameplay: self.open,
            exit_requested: self.exit_requested,
            state: self.build_ui_state(visual_visible),
        }
    }

    #[inline]
    fn open(&mut self, frame_index: u64) {
        self.open = true;
        if let Some(navigation) = self.navigation.as_mut() {
            navigation.reset_to_root();
        }
        self.awaiting_rebind = None;
        self.profile = newengine_input_bindings_runtime::input_bindings_profile_snapshot();
        self.flash_feedback(
            "UI surface",
            "Game simulation is paused by a modal UI node; gameplay input is captured by engine.ui",
            UiNodeMessageSeverity::Info,
        );
        audio(AudioFeedbackKind::UiOpen, frame_index);
    }

    #[inline]
    fn close(&mut self, frame_index: u64) {
        self.open = false;
        if let Some(navigation) = self.navigation.as_mut() {
            navigation.reset_to_root();
        }
        self.awaiting_rebind = None;
        self.flash_feedback(
            "Resume",
            "Returning to gameplay",
            UiNodeMessageSeverity::Success,
        );
        audio(AudioFeedbackKind::UiClose, frame_index);
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
        severity: UiNodeMessageSeverity,
    ) {
        self.feedback = Some(UiNodeSurfaceEventFeedback::new(title, detail, severity));
    }

    #[inline]
    fn flash_navigation_feedback(&mut self, feedback: &UiNodeFeedbackEvent) {
        self.feedback = Some(UiNodeSurfaceEventFeedback::from_navigation_feedback(
            feedback,
        ));
    }

    fn ensure_navigation_document_loaded(&mut self, frame_index: u64) -> bool {
        if self.navigation.is_some() {
            return true;
        }

        let should_attempt = self
            .document_last_attempt_frame
            .map(|last| frame_index.saturating_sub(last) >= 120)
            .unwrap_or(true);
        if !should_attempt {
            return false;
        }

        self.document_last_attempt_frame = Some(frame_index);
        let Some(document_ref) =
            crate::env_config::var(newengine_ui_navigation_api::ENGINE_PRIMARY_UI_DOCUMENT_REF_ENV)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        else {
            self.document_load_error = Some(
                "primary UI enabled without NEWENGINE_PRIMARY_UI_DOCUMENT_REF; project frontends belong in game.toml ui.presentation_flow"
                    .to_owned(),
            );
            return false;
        };
        match document::try_load_primary_ui_document(&document_ref) {
            Ok(navigation) => {
                newengine_ulog_api::ulog::info!(
                    "engine.ui.primary: compiled .neui UI surface available through engine.ui ref='{}'",
                    document_ref
                );
                self.document_load_error = None;
                self.navigation = Some(navigation);
                true
            }
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "engine.ui.primary: compiled .neui UI surface unavailable ref='{}' err='{}'",
                    document_ref,
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
            dim_opacity: 0.98 * a,
            blur_radius_px: 22.0 * a,
        }
    }
}

fn hovered_item_index(
    mouse_pos: Option<(f32, f32)>,
    surface_size_px: [u32; 2],
    item_count: usize,
    _animation_alpha: f32,
) -> Option<usize> {
    // The retained UI node is rendered as a normal retained UiSurfaceNode. Hit testing must
    // therefore use the same surface-node layout contract as the provider draw path,
    // not a special interface layout. Body line 0 is the page/status header;
    // UI items start at line 1.
    ui_surface_node_layout(
        surface_size_px,
        &[
            "retained".to_owned(),
            "modern".to_owned(),
            "rounded".to_owned(),
        ],
        &ui_surface_style(),
        item_count + 1,
        0,
    )
    .hit_item_index_after_header(mouse_pos, 1, item_count)
}

#[inline]
fn audio(kind: AudioFeedbackKind, frame_index: u64) {
    newengine_audio_client::emit_audio_feedback(kind, frame_index);
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
fn tone_from_navigation(tone: UiNodeNavigationTone) -> UiNodeTone {
    match tone {
        UiNodeNavigationTone::Normal => UiNodeTone::Normal,
        UiNodeNavigationTone::Accent => UiNodeTone::Accent,
        UiNodeNavigationTone::Danger => UiNodeTone::Danger,
        UiNodeNavigationTone::Disabled => UiNodeTone::Disabled,
    }
}

#[allow(clippy::field_reassign_with_default)]
fn ui_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.anchor = UiSurfaceAnchor::TopLeft;
    style.min_size_px = [520.0, 430.0];
    style.max_size_px = [760.0, 760.0];
    style.margin_px = [72.0, 28.0];
    style.padding_px = [38.0, 106.0, 38.0, 62.0];
    style.row_pitch_px = 30.0;
    style.panel_rgba = [6, 9, 17, 244];
    style.panel_header_rgba = [14, 19, 34, 246];
    style.accent_rgba = [108, 204, 255, 255];
    style.text_rgba = [238, 245, 255, 255];
    style.text_muted_rgba = [162, 178, 204, 255];
    style.border_rgba = [116, 190, 255, 70];
    style.backdrop_rgba = [0, 0, 0, 190];
    style.corner_radius_px = 22.0;
    style.border_px = 1.0;
    style.shadow_alpha = 0;
    style.font.stack = vec![
        "NorthStarSans".to_owned(),
        "Inter".to_owned(),
        "Segoe UI".to_owned(),
        "NotoSans".to_owned(),
    ];
    style.font.title_px = 30.0;
    style.font.body_px = 17.0;
    style.font.secondary_px = 14.0;
    style.normalized()
}

#[inline]
fn severity_from_navigation(severity: UiNodeFeedbackSeverity) -> UiNodeMessageSeverity {
    match severity {
        UiNodeFeedbackSeverity::Info => UiNodeMessageSeverity::Info,
        UiNodeFeedbackSeverity::Success => UiNodeMessageSeverity::Success,
        UiNodeFeedbackSeverity::Warning => UiNodeMessageSeverity::Warning,
        UiNodeFeedbackSeverity::Danger => UiNodeMessageSeverity::Danger,
    }
}

#[inline]
fn audio_feedback_from_route(audio_id: &str) -> AudioFeedbackKind {
    match audio_id {
        "ui.open" => AudioFeedbackKind::UiOpen,
        "ui.close" => AudioFeedbackKind::UiClose,
        "ui.navigate" => AudioFeedbackKind::UiNavigate,
        "ui.back" => AudioFeedbackKind::UiBack,
        "ui.rebind" => AudioFeedbackKind::UiRebind,
        "ui.error" => AudioFeedbackKind::UiError,
        "ui.confirm" => AudioFeedbackKind::UiConfirm,
        _ => AudioFeedbackKind::UiConfirm,
    }
}
