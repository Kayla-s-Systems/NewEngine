use newengine_ui_api::{UiSurfaceAnchor, UiSurfaceStyle, UI_THEME_NORTHSTAR_DEFAULT};

pub(crate) fn showcase_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.theme_id = UI_THEME_NORTHSTAR_DEFAULT.to_owned();
    style.anchor = UiSurfaceAnchor::Center;
    style.min_size_px = [980.0, 660.0];
    style.max_size_px = [1180.0, 700.0];
    style.margin_px = [28.0, 28.0];
    style.padding_px = [28.0, 48.0, 28.0, 28.0];
    style.row_pitch_px = 32.0;
    style.panel_rgba = [255, 255, 255, 255];
    style.panel_header_rgba = [245, 247, 250, 255];
    style.accent_rgba = [0, 113, 188, 255];
    style.text_rgba = [20, 24, 32, 255];
    style.text_muted_rgba = [88, 96, 112, 255];
    style.border_rgba = [210, 216, 224, 255];
    style.backdrop_rgba = [255, 255, 255, 255];
    style.corner_radius_px = 18.0;
    style.border_px = 1.0;
    style.shadow_alpha = 72;
    style.font.title_px = 28.0;
    style.font.body_px = 15.0;
    style.font.secondary_px = 12.5;
    style.normalized()
}
