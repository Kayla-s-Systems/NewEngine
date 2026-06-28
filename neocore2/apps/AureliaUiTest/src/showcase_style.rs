use newengine_ui_api::{UiSurfaceAnchor, UiSurfaceStyle, UI_THEME_NORTHSTAR_DEFAULT};

pub(crate) fn showcase_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.theme_id = UI_THEME_NORTHSTAR_DEFAULT.to_owned();
    style.anchor = UiSurfaceAnchor::TopLeft;
    style.min_size_px = [1280.0, 720.0];
    style.max_size_px = [1280.0, 720.0];
    style.margin_px = [0.0, 0.0];
    style.padding_px = [24.0, 48.0, 24.0, 24.0];
    style.row_pitch_px = 32.0;
    style.panel_rgba = [248, 250, 253, 255];
    style.panel_header_rgba = [255, 255, 255, 255];
    style.accent_rgba = [0, 113, 206, 255];
    style.text_rgba = [23, 32, 54, 255];
    style.text_muted_rgba = [91, 104, 126, 255];
    style.border_rgba = [218, 225, 235, 255];
    style.backdrop_rgba = [255, 255, 255, 255];
    style.corner_radius_px = 8.0;
    style.border_px = 1.0;
    style.shadow_alpha = 24;
    style.font.title_px = 30.0;
    style.font.body_px = 13.0;
    style.font.secondary_px = 11.5;
    style.normalized()
}
