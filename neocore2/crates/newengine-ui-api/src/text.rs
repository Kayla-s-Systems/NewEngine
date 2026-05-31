// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiFontStyle {
    /// Declarative font fallback stack. Providers may resolve this through
    /// `engine.ui.text`; the engine treats it as data, not concrete font files.
    pub stack: Vec<String>,
    /// Body text size in physical pixels before provider snapping.
    pub body_px: f32,
    /// Title text size in physical pixels before provider snapping.
    pub title_px: f32,
    /// Subtitle/footer text size in physical pixels before provider snapping.
    pub secondary_px: f32,
    /// Text baseline pitch in physical pixels. `0` means provider default.
    pub line_height_px: f32,
    /// Pixel-snap text quads. Modern editor themes normally keep this false;
    /// low-resolution fallback atlases may enable it explicitly.
    pub pixel_snap: bool,
}

impl Default for UiFontStyle {
    fn default() -> Self {
        Self {
            stack: vec![
                UI_FONT_ASSET_EDITOR_SANS.to_owned(),
                "Inter".to_owned(),
                "Segoe UI".to_owned(),
                "NotoSans".to_owned(),
                "NotoSansSymbols".to_owned(),
            ],
            body_px: 12.0,
            title_px: 15.0,
            secondary_px: 10.5,
            line_height_px: 16.0,
            pixel_snap: false,
        }
    }
}

impl UiFontStyle {
    #[inline]
    pub fn normalized(mut self) -> Self {
        if self.stack.is_empty() {
            self.stack = UiFontStyle::default().stack;
        }
        self.body_px = self.body_px.clamp(10.0, 48.0);
        self.title_px = self.title_px.clamp(14.0, 72.0);
        self.secondary_px = self.secondary_px.clamp(9.0, 36.0);
        self.line_height_px = self.line_height_px.clamp(0.0, 96.0);
        self
    }
}
