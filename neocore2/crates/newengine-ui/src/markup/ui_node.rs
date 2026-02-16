#![forbid(unsafe_op_in_unsafe_fn)]

use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiIconSide {
    Left,
    Right,
}

impl UiIconSide {
    #[inline]
    pub(crate) fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "right" | "end" | "after" => Self::Right,
            _ => Self::Left,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum UiNode {
    Ui {
        children: Vec<UiNode>,
    },
    TopBar {
        children: Vec<UiNode>,
    },
    Window {
        title: String,
        open: bool,
        children: Vec<UiNode>,
    },
    Row {
        children: Vec<UiNode>,
    },
    Column {
        children: Vec<UiNode>,
    },

    Label {
        id: Option<String>,
        text: String,
        icon: Option<String>,
        icon_side: UiIconSide,
        icon_size: Option<f32>,
    },
    Button {
        id: String,
        text: String,
        icon: Option<String>,
        icon_side: UiIconSide,
        icon_size: Option<f32>,
        on_click: SmallVec<[String; 2]>,
    },
    TextBox {
        id: String,
        hint: String,
        bind: String,
        multiline: bool,
        on_change: SmallVec<[String; 2]>,
        on_submit: SmallVec<[String; 2]>,
    },

    Checkbox {
        id: String,
        text: String,
        bind: String,
        on_change: SmallVec<[String; 2]>,
    },

    Select {
        id: String,
        bind: String,
        /// (value, label)
        options: Vec<(String, String)>,
        on_change: SmallVec<[String; 2]>,
    },

    Separator,

    Scroll {
        id: Option<String>,
        children: Vec<UiNode>,
    },

    /// Repeat children for each JSON object in `items`.
    ///
    /// - `items` is a key in UiState.vars that must contain a JSON array.
    /// - `as_name` is a variable prefix, e.g. "p" => "$p.id", "$p.name".
    Repeat {
        items: String,
        as_name: String,
        children: Vec<UiNode>,
    },

    Spacer,

    /// Draw a texture as an UI element.
    ///
    /// `tex` must resolve to an `egui::TextureId::User(u64)`.
    Image {
        id: Option<String>,
        tex: String,
        /// Desired size in points (logical units). If omitted, 16x16 is used.
        size: Option<[f32; 2]>,
        /// Optional tint as RGBA hex: "#RRGGBBAA" or "#RRGGBB".
        tint: Option<String>,
    },

    Unknown {
        tag: String,
        children: Vec<UiNode>,
    },
}