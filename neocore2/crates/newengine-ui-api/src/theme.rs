// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFontRole {
    Title,
    Body,
    Secondary,
    Code,
    Icon,
}

impl Default for UiFontRole {
    #[inline]
    fn default() -> Self { Self::Body }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeFontToken {
    pub id: String,
    pub role: UiFontRole,
    /// Provider-neutral family stack. A provider may map these names to real
    /// font assets through `engine.ui.text`; runtime never sees font files.
    pub family_stack: Vec<String>,
    pub size_px: f32,
    pub line_height_px: f32,
    pub weight: u16,
    pub pixel_snap: bool,
}

impl Default for UiThemeFontToken {
    fn default() -> Self {
        Self {
            id: "body".to_owned(),
            role: UiFontRole::Body,
            family_stack: UiFontStyle::default().stack,
            size_px: 18.0,
            line_height_px: 24.0,
            weight: 500,
            pixel_snap: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeComponentStyle {
    pub component_id: String,
    pub font_token: String,
    pub min_size_px: [f32; 2],
    pub padding_px: [f32; 4],
    pub row_pitch_px: f32,
    pub interactive: bool,
    pub paint_layer: i32,
}

impl Default for UiThemeComponentStyle {
    fn default() -> Self {
        Self {
            component_id: UI_COMPONENT_PANEL.to_owned(),
            font_token: "body".to_owned(),
            min_size_px: [260.0, 120.0],
            padding_px: [28.0, 22.0, 28.0, 22.0],
            row_pitch_px: 26.0,
            interactive: false,
            paint_layer: 0,
        }
    }
}


/// Declarative style rule for authored `.neui/theme` assets.
///
/// Example selectors: `button.primary:hover`, `input.error:focused`,
/// `tree.row:selected`, `scrollbar.thumb:active`, `panel.dock.right`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeStyleRule {
    pub selector: String,
    pub props: BTreeMap<String, serde_json::Value>,
}

impl Default for UiThemeStyleRule {
    fn default() -> Self {
        Self {
            selector: String::new(),
            props: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeDefinition {
    pub id: String,
    pub display_name: String,
    pub default_component: String,
    pub fonts: BTreeMap<String, UiThemeFontToken>,
    pub components: BTreeMap<String, UiThemeComponentStyle>,
    pub tokens: BTreeMap<String, serde_json::Value>,
    pub style_rules: Vec<UiThemeStyleRule>,
    pub base_style: UiSurfaceStyle,
}

impl Default for UiThemeDefinition {
    fn default() -> Self {
        let mut fonts = BTreeMap::new();
        fonts.insert("title".to_owned(), UiThemeFontToken {
            id: "title".to_owned(),
            role: UiFontRole::Title,
            size_px: 30.0,
            line_height_px: 36.0,
            weight: 700,
            ..UiThemeFontToken::default()
        });
        fonts.insert("body".to_owned(), UiThemeFontToken::default());
        fonts.insert("secondary".to_owned(), UiThemeFontToken {
            id: "secondary".to_owned(),
            role: UiFontRole::Secondary,
            size_px: 15.0,
            line_height_px: 20.0,
            weight: 500,
            ..UiThemeFontToken::default()
        });
        fonts.insert("code".to_owned(), UiThemeFontToken {
            id: "code".to_owned(),
            role: UiFontRole::Code,
            family_stack: vec!["NorthStarMono".to_owned(), "CascadiaMono".to_owned(), "NotoSansMono".to_owned()],
            size_px: 16.0,
            line_height_px: 22.0,
            weight: 500,
            ..UiThemeFontToken::default()
        });

        let mut components = BTreeMap::new();
        components.insert(UI_COMPONENT_PANEL.to_owned(), UiThemeComponentStyle::default());
        components.insert(UI_COMPONENT_STACK.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_STACK.to_owned(),
            row_pitch_px: 26.0,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_ROW.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_ROW.to_owned(),
            row_pitch_px: 26.0,
            interactive: true,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_TEXT.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_TEXT.to_owned(),
            font_token: "body".to_owned(),
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_ACTION.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_ACTION.to_owned(),
            font_token: "body".to_owned(),
            interactive: true,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_SPACER.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_SPACER.to_owned(),
            min_size_px: [1.0, 10.0],
            ..UiThemeComponentStyle::default()
        });

        Self {
            id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            display_name: "North Star Default".to_owned(),
            default_component: UI_COMPONENT_PANEL.to_owned(),
            fonts,
            components,
            tokens: BTreeMap::new(),
            style_rules: Vec::new(),
            base_style: UiSurfaceStyle::default(),
        }
    }
}
