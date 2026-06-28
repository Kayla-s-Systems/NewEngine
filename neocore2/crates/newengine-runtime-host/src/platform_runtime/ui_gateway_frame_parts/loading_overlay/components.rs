use super::*;

const STARTUP_BG_TEXTURE: &str = "tmp/bg.png";
const STARTUP_LOGO_SPRITE: &str = "tmp/logo.png";
const STARTUP_LOGO_SPRITE_COLUMNS: u32 = 14;
const STARTUP_LOGO_SPRITE_ROWS: u32 = 14;
const STARTUP_LOGO_FRAME_WIDTH: u32 = 256;
const STARTUP_LOGO_FRAME_HEIGHT: u32 = 256;
const STARTUP_LOGO_FRAME_COUNT: u64 = 177;
const STARTUP_LOGO_FPS: u32 = 30;

pub(super) fn loading_overlay_components(
    progress_01: f32,
    progress_percent: f32,
    frame_index: u64,
) -> Vec<UiComponentNode> {
    let mut background =
        UiComponentNode::text("loading.background", "").tagged("startup-background");
    background.component_id = newengine_ui_api::UI_COMPONENT_EXTERNAL_TEXTURE.to_owned();
    background.icon = Some(STARTUP_BG_TEXTURE.to_owned());
    background
        .props
        .insert("texture".to_owned(), serde_json::json!(STARTUP_BG_TEXTURE));
    background.props.insert(
        "asset_path".to_owned(),
        serde_json::json!(STARTUP_BG_TEXTURE),
    );
    background
        .props
        .insert("fit".to_owned(), serde_json::json!("cover"));
    background
        .props
        .insert("layer".to_owned(), serde_json::json!("background"));

    let mut logo = UiComponentNode::text("loading.logo_sprite", "")
        .tagged("startup-logo")
        .tagged("sprite-animation");
    logo.component_id = newengine_ui_api::UI_COMPONENT_EXTERNAL_TEXTURE.to_owned();
    logo.icon = Some(STARTUP_LOGO_SPRITE.to_owned());
    logo.props
        .insert("texture".to_owned(), serde_json::json!(STARTUP_LOGO_SPRITE));
    logo.props.insert(
        "asset_path".to_owned(),
        serde_json::json!(STARTUP_LOGO_SPRITE),
    );
    logo.props
        .insert("anchor".to_owned(), serde_json::json!("center"));
    logo.props.insert(
        "sprite_columns".to_owned(),
        serde_json::json!(STARTUP_LOGO_SPRITE_COLUMNS),
    );
    logo.props.insert(
        "sprite_rows".to_owned(),
        serde_json::json!(STARTUP_LOGO_SPRITE_ROWS),
    );
    logo.props.insert(
        "frame_width".to_owned(),
        serde_json::json!(STARTUP_LOGO_FRAME_WIDTH),
    );
    logo.props.insert(
        "frame_height".to_owned(),
        serde_json::json!(STARTUP_LOGO_FRAME_HEIGHT),
    );
    logo.props.insert(
        "frame_count".to_owned(),
        serde_json::json!(STARTUP_LOGO_FRAME_COUNT),
    );
    logo.props.insert(
        "frame_index".to_owned(),
        serde_json::json!(logo_sprite_frame(frame_index)),
    );
    logo.props
        .insert("fps".to_owned(), serde_json::json!(STARTUP_LOGO_FPS));
    logo.props
        .insert("loop".to_owned(), serde_json::json!(false));
    logo.props
        .insert("freeze_last_frame".to_owned(), serde_json::json!(true));

    let mut progress = UiComponentNode::row("loading.progress_bar", "")
        .with_value(format!("{progress_percent:.0}%"))
        .tagged("progress")
        .tagged("progress-bar");
    progress.component_id = "progress_bar".to_owned();
    progress
        .props
        .insert("progress_01".to_owned(), serde_json::json!(progress_01));
    progress
        .props
        .insert("percent".to_owned(), serde_json::json!(progress_percent));

    vec![background, logo, progress]
}

fn logo_sprite_frame(frame_index: u64) -> u64 {
    frame_index.min(STARTUP_LOGO_FRAME_COUNT.saturating_sub(1))
}

pub(super) fn error_overlay_components(status: &ScreenOverlayStatus) -> Vec<UiComponentNode> {
    let mut reason = UiComponentNode::row("error.reason", "Reason")
        .with_value(format!("{:?}", status.reason))
        .tagged("error-reason")
        .tagged("diagnostic");
    reason.component_id = "status_badge".to_owned();

    let mut detail = UiComponentNode::text("error.detail", status.detail.clone())
        .with_tone(newengine_ui_api::UiNodeTone::Disabled)
        .tagged("error-detail")
        .tagged("diagnostic-body");
    detail
        .props
        .insert("selectable".to_owned(), serde_json::json!(true));

    vec![
        UiComponentNode::text("error.title", status.title.clone())
            .with_tone(newengine_ui_api::UiNodeTone::Danger)
            .tagged("error-title"),
        UiComponentNode::text("error.status", status.status.clone())
            .with_tone(newengine_ui_api::UiNodeTone::Accent)
            .tagged("error-status"),
        reason,
        detail,
        UiComponentNode::text(
            "error.footer",
            "NORTHSTAR // renderer failure captured; process held for diagnostics.".to_owned(),
        )
        .with_tone(newengine_ui_api::UiNodeTone::Disabled)
        .tagged("error-footer"),
    ]
}
