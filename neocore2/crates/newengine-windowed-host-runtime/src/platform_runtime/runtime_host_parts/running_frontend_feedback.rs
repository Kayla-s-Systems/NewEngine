use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use newengine_ui_api::{UiEventDispatchFrame, UiInputFrame};

const FRONTEND_KEYCAP_FEEDBACK_DURATION: Duration = Duration::from_millis(360);
const FRONTEND_EXIT_FEEDBACK_HOLD: Duration = Duration::from_millis(240);

pub(super) fn ui_dispatch_requests_exit(frame: &UiEventDispatchFrame) -> bool {
    frame.actions.iter().any(|action| {
        action.trigger == newengine_ui_api::UiNodeEventTrigger::Click
            && matches!(
                action.action_id.as_str(),
                "engine.lifecycle.exit" | "engine.exit.request" | "app.exit"
            )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FrontendKeycapKind {
    Select,
    Back,
}

#[derive(Clone, Debug)]
struct FrontendKeycapFeedback {
    kind: FrontendKeycapKind,
    label: String,
    started_at: Instant,
}

fn frontend_keycap_feedback() -> &'static Mutex<Option<FrontendKeycapFeedback>> {
    static FEEDBACK: OnceLock<Mutex<Option<FrontendKeycapFeedback>>> = OnceLock::new();
    FEEDBACK.get_or_init(|| Mutex::new(None))
}

pub(super) fn begin_frontend_keycap_feedback(kind: FrontendKeycapKind, label: impl Into<String>) {
    *frontend_keycap_feedback()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(FrontendKeycapFeedback {
        kind,
        label: label.into(),
        started_at: Instant::now(),
    });
}

fn frontend_exit_pending() -> &'static Mutex<Option<Instant>> {
    static PENDING: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

pub(super) fn frontend_exit_feedback_due(requested_now: bool) -> bool {
    let mut pending = frontend_exit_pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if requested_now && pending.is_none() {
        *pending = Some(Instant::now());
    }
    let due = pending
        .as_ref()
        .is_some_and(|started| started.elapsed() >= FRONTEND_EXIT_FEEDBACK_HOLD);
    if due {
        *pending = None;
    }
    due
}

pub(super) fn update_frontend_keycap_feedback(
    input: Option<&UiInputFrame>,
    dispatch: Option<&UiEventDispatchFrame>,
    presentation_state: Option<&str>,
) {
    if let Some(action) = dispatch.and_then(|frame| {
        frame.actions.iter().find(|action| {
            matches!(
                action.trigger,
                newengine_ui_api::UiNodeEventTrigger::Click
                    | newengine_ui_api::UiNodeEventTrigger::ValueChanged
            )
        })
    }) {
        let (kind, label) = frontend_action_keycap(action.action_id.as_str());
        begin_frontend_keycap_feedback(kind, label);
        return;
    }
    let Some(input) = input else {
        return;
    };
    if input.is_key_pressed(newengine_ui_api::keys::KEY_E) {
        begin_frontend_keycap_feedback(FrontendKeycapKind::Select, "SELECT");
    } else if input.is_key_pressed(newengine_ui_api::keys::ESCAPE) {
        let label = if presentation_state == Some("main_menu") {
            "EXIT"
        } else {
            "BACK"
        };
        begin_frontend_keycap_feedback(FrontendKeycapKind::Back, label);
    }
}

pub(super) fn frontend_action_keycap(action_id: &str) -> (FrontendKeycapKind, &'static str) {
    match action_id {
        "engine.lifecycle.exit" | "engine.exit.request" | "app.exit" => {
            (FrontendKeycapKind::Back, "EXITING")
        }
        "ui.back" => (FrontendKeycapKind::Back, "RETURN"),
        "game.start" => (FrontendKeycapKind::Select, "START"),
        "engine.settings.open" | "game.credits" => (FrontendKeycapKind::Select, "OPEN"),
        "settings.apply" => (FrontendKeycapKind::Select, "APPLY"),
        action if action.starts_with("settings.") => (FrontendKeycapKind::Select, "CHANGE"),
        _ => (FrontendKeycapKind::Select, "SELECT"),
    }
}

pub(super) fn animate_frontend_keycap_feedback(draw: &mut newengine_ui_api::UiDrawList) {
    let feedback = {
        let mut feedback = frontend_keycap_feedback()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(current) = feedback.as_ref() else {
            return;
        };
        if current.started_at.elapsed() >= FRONTEND_KEYCAP_FEEDBACK_DURATION {
            *feedback = None;
            return;
        }
        current.clone()
    };
    let elapsed = feedback.started_at.elapsed();
    let press = frontend_keycap_press_amount(elapsed);
    let key_token = match feedback.kind {
        FrontendKeycapKind::Select => ".hint.select.",
        FrontendKeycapKind::Back => ".hint.back.",
    };

    let mut transformed = Vec::with_capacity(draw.paint.commands.len() + 2);
    for mut command in std::mem::take(&mut draw.paint.commands) {
        match &mut command {
            newengine_ui_api::UiPaintCommand::Image(image)
                if image.node.node_id.contains(key_token)
                    && image.node.node_id.ends_with("keycap") =>
            {
                let original = image.rect;
                if press > 0.001 {
                    let mut well_node = image.node.clone();
                    well_node.node_id = format!("{}.pressed-well", well_node.node_id);
                    well_node.role = "keycap-pressed-well".to_owned();
                    well_node.z_index = well_node.z_index.saturating_sub(1);
                    transformed.push(newengine_ui_api::UiPaintCommand::Rect(
                        newengine_ui_api::UiRectPaintCommand {
                            node: well_node,
                            rect: [
                                original[0] + 1.0,
                                original[1] + 2.0,
                                (original[2] - 2.0).max(1.0),
                                (original[3] - 2.0).max(1.0),
                            ],
                            color: lerp_rgba_u32(
                                rgba_u32(32, 20, 12, 90),
                                rgba_u32(124, 78, 43, 225),
                                press,
                            ),
                            clip_rect: image.clip_rect,
                        },
                    ));
                }

                let target_w = original[2] * (1.0 - 0.08 * press);
                let target_h = original[3] * (1.0 - 0.18 * press);
                image.rect[0] = original[0] + (original[2] - target_w) * 0.5;
                // Bottom-anchored compression plus a small downward travel creates
                // a readable physical key press at 1600x900.
                image.rect[1] = original[1] + (original[3] - target_h) + 1.5 * press;
                image.rect[2] = target_w.max(1.0);
                image.rect[3] = target_h.max(1.0);
                image.tint_rgba =
                    lerp_rgba_u32(image.tint_rgba, rgba_u32(255, 216, 174, 255), 0.92 * press);
            }
            newengine_ui_api::UiPaintCommand::Text(text)
                if text.node.node_id.contains(key_token) && text.node.node_id.ends_with("text") =>
            {
                text.text = feedback.label.clone();
                text.rect[0] += 1.0 * press;
                text.rect[1] += 3.5 * press;
                text.color = lerp_rgba_u32(text.color, rgba_u32(255, 232, 204, 255), 0.96 * press);
                text.letter_spacing_px += 0.28 * press;
            }
            _ => {}
        }
        transformed.push(command);
    }
    draw.paint.commands = transformed;
}

pub(super) fn frontend_keycap_press_amount(elapsed: Duration) -> f32 {
    let elapsed_ms = elapsed.as_secs_f32() * 1_000.0;
    const ATTACK_MS: f32 = 45.0;
    const HOLD_UNTIL_MS: f32 = 190.0;
    let duration_ms = FRONTEND_KEYCAP_FEEDBACK_DURATION.as_secs_f32() * 1_000.0;
    if elapsed_ms <= ATTACK_MS {
        smoothstep01(elapsed_ms / ATTACK_MS)
    } else if elapsed_ms <= HOLD_UNTIL_MS {
        1.0
    } else {
        smoothstep01((duration_ms - elapsed_ms) / (duration_ms - HOLD_UNTIL_MS).max(1.0))
    }
}

fn smoothstep01(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

const fn rgba_u32(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
}

fn lerp_rgba_u32(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| -> u8 {
        let a = ((from >> shift) & 0xff) as f32;
        let b = ((to >> shift) & 0xff) as f32;
        (a + (b - a) * amount).round().clamp(0.0, 255.0) as u8
    };
    rgba_u32(channel(0), channel(8), channel(16), channel(24))
}
