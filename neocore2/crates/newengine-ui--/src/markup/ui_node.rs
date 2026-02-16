#![forbid(unsafe_op_in_unsafe_fn)]

use smallvec::SmallVec;

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
    },
    Button {
        id: String,
        text: String,
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

    Unknown {
        tag: String,
        children: Vec<UiNode>,
    },
}