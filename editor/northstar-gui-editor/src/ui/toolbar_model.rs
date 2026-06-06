#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarButton {
    pub x: i32,
    pub label: &'static str,
    pub icon: &'static str,
    pub hint: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchBox {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub fn buttons() -> &'static [ToolbarButton] {
    &[
        ToolbarButton { x: 14, label: "Back", icon: "Back", hint: "Back" },
        ToolbarButton { x: 58, label: "Open", icon: "Open", hint: "Open" },
        ToolbarButton { x: 102, label: "Save", icon: "Save", hint: "Save  Ctrl+S" },
        ToolbarButton { x: 146, label: "Undo", icon: "Undo", hint: "Undo  Ctrl+Z" },
        ToolbarButton { x: 190, label: "Redo", icon: "Redo", hint: "Redo  Ctrl+Y" },
        ToolbarButton { x: 234, label: "Validate", icon: "Ok", hint: "Validate" },
        ToolbarButton { x: 278, label: "Reload Types", icon: "Reload", hint: "Reload tool/type routes" },
        ToolbarButton { x: 322, label: "Preview", icon: "View", hint: "Preview" },
        ToolbarButton { x: 366, label: "Theme", icon: "Theme", hint: "Theme" },
    ]
}

pub fn button_width(_label: &str) -> i32 {
    34
}

pub fn button_for_label(label: &str) -> Option<ToolbarButton> {
    buttons().iter().copied().find(|button| button.label == label)
}

pub fn search_box(client_right: i32) -> SearchBox {
    SearchBox { left: client_right - 420, top: 42, right: client_right - 24, bottom: 68 }
}
