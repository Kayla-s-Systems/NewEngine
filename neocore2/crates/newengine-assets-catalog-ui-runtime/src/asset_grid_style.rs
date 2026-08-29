use super::*;

pub(crate) fn assets_catalog_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.anchor = UiSurfaceAnchor::BottomLeft;
    style.min_size_px = [960.0, 248.0];
    style.max_size_px = [4096.0, 320.0];
    style.margin_px = [8.0, 30.0];
    style.padding_px = [14.0, 44.0, 14.0, 24.0];
    style.row_pitch_px = 20.0;
    style.panel_rgba = [6, 10, 16, 252];
    style.panel_header_rgba = [9, 15, 24, 252];
    style.accent_rgba = [89, 164, 255, 255];
    style.text_rgba = [225, 232, 242, 255];
    style.text_muted_rgba = [137, 150, 168, 255];
    style.danger_rgba = [238, 110, 88, 255];
    style.border_rgba = [72, 91, 116, 135];
    style.backdrop_rgba = [0, 0, 0, 36];
    style.shadow_alpha = 82;
    style.corner_radius_px = 7.0;
    style.border_px = 1.0;
    style.font.stack = vec![
        UI_FONT_ASSET_EDITOR_SANS.to_owned(),
        "Inter".to_owned(),
        "Segoe UI".to_owned(),
        "NotoSans".to_owned(),
    ];
    style.font.title_px = 14.0;
    style.font.body_px = 10.0;
    style.font.secondary_px = 9.0;
    style.row_even_alpha = 8;
    style.row_odd_alpha = 3;
    style.normalized()
}
