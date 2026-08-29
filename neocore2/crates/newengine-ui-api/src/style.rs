// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSurfaceAnchor {
    TopLeft,
    TopRight,
    Center,
    BottomLeft,
    BottomRight,
}

impl Default for UiSurfaceAnchor {
    #[inline]
    fn default() -> Self { Self::TopLeft }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInteractionState {
    Normal,
    Hovered,
    Active,
    Pressed,
    Focused,
    Selected,
    Error,
    Disabled,
}

impl Default for UiInteractionState {
    #[inline]
    fn default() -> Self { Self::Normal }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiInteractionStyle {
    pub normal_rgba: [u8; 4],
    pub hover_rgba: [u8; 4],
    pub active_rgba: [u8; 4],
    pub pressed_rgba: [u8; 4],
    pub disabled_rgba: [u8; 4],
    pub focus_ring_rgba: [u8; 4],
    pub tooltip_rgba: [u8; 4],
    pub tooltip_text_rgba: [u8; 4],
    pub transition_ms: u16,
    pub tooltip_delay_ms: u16,
}

impl Default for UiInteractionStyle {
    fn default() -> Self {
        Self {
            normal_rgba: [248, 250, 252, 246],
            hover_rgba: [232, 240, 252, 255],
            active_rgba: [214, 231, 255, 255],
            pressed_rgba: [190, 218, 255, 255],
            disabled_rgba: [226, 232, 240, 210],
            focus_ring_rgba: [37, 99, 235, 190],
            tooltip_rgba: [255, 255, 255, 252],
            tooltip_text_rgba: [15, 23, 42, 255],
            transition_ms: 90,
            tooltip_delay_ms: 280,
        }
    }
}


/// Provider-neutral resolved style produced by the UI style cascade.
///
/// Cascade order:
/// `theme tokens -> component default -> style_ref -> style_tags -> state_tags -> explicit props`.
/// Providers may serialize this into `UiLayoutBox::computed_style` so paint,
/// hit-test overlays and diagnostics all inspect the same resolved state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiComputedStyle {
    pub theme_id: String,
    pub style_ref: Option<String>,
    pub component_id: String,
    pub role: String,
    pub font_token: String,
    pub layout_kind: String,
    pub selector: String,
    pub style_tags: Vec<String>,
    pub state_tags: Vec<String>,
    pub state: UiInteractionState,
    pub action_id: Option<String>,
    pub z_path: Vec<String>,
    pub background_rgba: [u8; 4],
    pub foreground_rgba: [u8; 4],
    pub muted_rgba: [u8; 4],
    pub accent_rgba: [u8; 4],
    pub border_rgba: [u8; 4],
    pub focus_ring_rgba: [u8; 4],
    pub danger_rgba: [u8; 4],
    pub shadow_rgba: [u8; 4],
    pub corner_radius_px: f32,
    pub border_px: f32,
    pub padding_px: [f32; 4],
    pub row_pitch_px: f32,
    pub opacity: f32,
    pub transition_ms: u16,
}

impl Default for UiComputedStyle {
    fn default() -> Self {
        let base = UiSurfaceStyle::default();
        Self {
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            style_ref: None,
            component_id: UI_COMPONENT_PANEL.to_owned(),
            role: UI_COMPONENT_PANEL.to_owned(),
            font_token: "body".to_owned(),
            layout_kind: "leaf".to_owned(),
            selector: "panel".to_owned(),
            style_tags: Vec::new(),
            state_tags: Vec::new(),
            state: UiInteractionState::Normal,
            action_id: None,
            z_path: Vec::new(),
            background_rgba: base.panel_rgba,
            foreground_rgba: base.text_rgba,
            muted_rgba: base.text_muted_rgba,
            accent_rgba: base.accent_rgba,
            border_rgba: base.border_rgba,
            focus_ring_rgba: base.interaction.focus_ring_rgba,
            danger_rgba: base.danger_rgba,
            shadow_rgba: [0, 0, 0, base.shadow_alpha],
            corner_radius_px: base.corner_radius_px,
            border_px: base.border_px,
            padding_px: base.padding_px,
            row_pitch_px: base.row_pitch_px,
            opacity: 1.0,
            transition_ms: base.interaction.transition_ms,
        }
    }
}

impl UiComputedStyle {
    #[inline]
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiTooltipState {
    pub visible: bool,
    pub owner_surface: String,
    pub node_id: String,
    pub text: String,
    pub detail: String,
    pub anchor_px: [f32; 2],
}

impl Default for UiTooltipState {
    fn default() -> Self {
        Self {
            visible: false,
            owner_surface: String::new(),
            node_id: String::new(),
            text: String::new(),
            detail: String::new(),
            anchor_px: [0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceStyle {
    pub theme_id: String,
    pub font: UiFontStyle,
    pub accent_rgba: [u8; 4],
    pub panel_rgba: [u8; 4],
    pub panel_header_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub text_muted_rgba: [u8; 4],
    pub danger_rgba: [u8; 4],
    pub interaction: UiInteractionStyle,
    pub row_even_alpha: u8,
    pub row_odd_alpha: u8,
    pub shadow_alpha: u8,
    /// Rounded surface radius in physical pixels. Providers may approximate it
    /// when the active renderer has no signed-distance/AA UI shader yet.
    pub corner_radius_px: f32,
    /// Thin modern surface outline. This is a style token, not a hardcoded provider color.
    pub border_rgba: [u8; 4],
    pub border_px: f32,
    /// Modal backdrop tint. UI state owns whether it is active; provider only paints it.
    pub backdrop_rgba: [u8; 4],
    pub anchor: UiSurfaceAnchor,
    pub min_size_px: [f32; 2],
    pub max_size_px: [f32; 2],
    pub margin_px: [f32; 2],
    pub padding_px: [f32; 4],
    pub row_pitch_px: f32,
}

impl Default for UiSurfaceStyle {
    fn default() -> Self {
        Self {
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            font: UiFontStyle::default(),
            accent_rgba: [37, 99, 235, 255],
            panel_rgba: [255, 255, 255, 248],
            panel_header_rgba: [241, 245, 249, 252],
            text_rgba: [15, 23, 42, 255],
            text_muted_rgba: [71, 85, 105, 255],
            danger_rgba: [220, 38, 38, 255],
            interaction: UiInteractionStyle::default(),
            row_even_alpha: 22,
            row_odd_alpha: 12,
            shadow_alpha: 54,
            corner_radius_px: 7.0,
            border_rgba: [203, 213, 225, 190],
            border_px: 1.0,
            backdrop_rgba: [226, 232, 240, 118],
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [260.0, 120.0],
            max_size_px: [560.0, 420.0],
            margin_px: [12.0, 12.0],
            padding_px: [12.0, 34.0, 12.0, 18.0],
            row_pitch_px: 0.0,
        }
    }
}

impl UiSurfaceStyle {
    #[inline]
    pub fn normalized(mut self) -> Self {
        self.font = self.font.normalized();
        if self.theme_id.trim().is_empty() {
            self.theme_id = UI_THEME_NORTHSTAR_DEFAULT.to_owned();
        }
        self.min_size_px[0] = self.min_size_px[0].clamp(96.0, 4096.0);
        self.min_size_px[1] = self.min_size_px[1].clamp(64.0, 4096.0);
        self.max_size_px[0] = self.max_size_px[0].max(self.min_size_px[0]).clamp(96.0, 4096.0);
        self.max_size_px[1] = self.max_size_px[1].max(self.min_size_px[1]).clamp(64.0, 4096.0);
        self.margin_px[0] = self.margin_px[0].clamp(0.0, 512.0);
        self.margin_px[1] = self.margin_px[1].clamp(0.0, 512.0);
        for value in self.padding_px.iter_mut() {
            *value = value.clamp(0.0, 512.0);
        }
        self.row_pitch_px = self.row_pitch_px.clamp(0.0, 256.0);
        self.corner_radius_px = self.corner_radius_px.clamp(0.0, 64.0);
        self.border_px = self.border_px.clamp(0.0, 8.0);
        self
    }
}
