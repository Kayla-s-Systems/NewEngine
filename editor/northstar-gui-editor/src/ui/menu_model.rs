#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopMenuItem {
    pub x: i32,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    Toolbar(&'static str),
    SetViewMode(&'static str),
    FocusFilter,
    ClearFilter,
    ResetLayout,
    SelectPanel(PanelTarget),
    OpenModal(ModalTarget),
    SubmenuPending(&'static str),
    Unknown(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTarget {
    Workspace,
    Files,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalTarget {
    About,
    LoadTools,
    Doctor,
}

pub fn default_view_mode() -> &'static str {
    "Таблица"
}

pub fn top_menu_items() -> &'static [TopMenuItem] {
    &[
        TopMenuItem {
            x: 10,
            label: "File",
        },
        TopMenuItem {
            x: 58,
            label: "Edit",
        },
        TopMenuItem {
            x: 106,
            label: "View",
        },
        TopMenuItem {
            x: 160,
            label: "Tools",
        },
        TopMenuItem {
            x: 222,
            label: "Window",
        },
        TopMenuItem {
            x: 302,
            label: "Help",
        },
    ]
}

pub fn top_menu_item_width(label: &str) -> i32 {
    (label.len() as i32 * 8).max(36) + 14
}

pub fn dropdown_items(menu: &'static str) -> &'static [&'static str] {
    match menu {
        "File" => &["Open", "Save", "Validate"],
        "Edit" => &["Undo", "Redo", "-", "Focus Filter", "Clear Filter"],
        "View" => &[
            "Раскладка >",
            "Сортировка >",
            "Группировка >",
            "-",
            "Обычные значки",
            "Мелкие значки",
            "Список",
            "Таблица",
            "Плитка",
            "-",
            "Переход >",
            "Preview",
            "Theme",
        ],
        "Tools" => &["Reload Types", "Load Tools...", "Doctor"],
        "Window" => &["Reset Layout", "Workspace", "Files", "Inspector"],
        "Help" => &["About"],
        _ => &[],
    }
}

pub fn dropdown_width(menu: &'static str) -> i32 {
    let max_label = dropdown_items(menu)
        .iter()
        .map(|item| item.len() as i32)
        .max()
        .unwrap_or(8);
    (max_label * 8 + 58).max(168)
}

pub fn dropdown_height(menu: &'static str) -> i32 {
    let mut height = 8;
    for item in dropdown_items(menu) {
        height += if is_separator(item) { 8 } else { 24 };
    }
    height
}

pub fn is_separator(item: &str) -> bool {
    item == "-"
}

pub fn is_submenu(item: &str) -> bool {
    item.ends_with('>')
}

pub fn clean_item_label(item: &str) -> &str {
    item.trim_end_matches(" >")
}

pub fn is_view_mode(item: &str) -> bool {
    matches!(
        item,
        "Обычные значки" | "Мелкие значки" | "Список" | "Таблица" | "Плитка"
    )
}

pub fn classify_menu_item(item: &'static str) -> MenuCommand {
    match item {
        "Open" | "Preview" | "Save" | "Undo" | "Redo" | "Validate" | "Reload Types" | "Theme" => {
            MenuCommand::Toolbar(item)
        }
        "Обычные значки" | "Мелкие значки" | "Список" | "Таблица" | "Плитка" => {
            MenuCommand::SetViewMode(item)
        }
        "Раскладка >" | "Сортировка >" | "Группировка >" | "Переход >" => {
            MenuCommand::SubmenuPending(item)
        }
        "Focus Filter" => MenuCommand::FocusFilter,
        "Clear Filter" => MenuCommand::ClearFilter,
        "Reset Layout" => MenuCommand::ResetLayout,
        "Workspace" => MenuCommand::SelectPanel(PanelTarget::Workspace),
        "Files" => MenuCommand::SelectPanel(PanelTarget::Files),
        "Inspector" => MenuCommand::SelectPanel(PanelTarget::Inspector),
        "Load Tools..." => MenuCommand::OpenModal(ModalTarget::LoadTools),
        "Doctor" => MenuCommand::OpenModal(ModalTarget::Doctor),
        "About" => MenuCommand::OpenModal(ModalTarget::About),
        other => MenuCommand::Unknown(other),
    }
}
