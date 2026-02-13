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

    Spacer,

    Unknown {
        tag: String,
        children: Vec<UiNode>,
    },
}