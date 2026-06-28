use super::*;

pub(super) static UI_STATE: OnceLock<Mutex<UiState>> = OnceLock::new();

#[derive(Debug, Clone)]
pub(super) struct UiState {
    pub(super) root: String,
    pub(super) selected_path: String,
    pub(super) expanded_paths: Vec<String>,
    pub(super) provider_count: usize,
    pub(super) capability_count: usize,
    pub(super) format_type_count: usize,
    pub(super) preview_provider_count: usize,
    pub(super) provider_ids: Vec<String>,
    pub(super) format_type_ids: Vec<String>,
    pub(super) tool_routes: Vec<ToolRouteDescriptor>,
    pub(super) selected_row: usize,
    pub(super) hover_row: Option<usize>,
    pub(super) selected_tree: usize,
    pub(super) tree_scroll_rows: usize,
    pub(super) scroll_rows: usize,
    pub(super) inspector_scroll_rows: usize,
    pub(super) hover_panel: HoverPanel,
    pub(super) menu_hover: Option<&'static str>,
    pub(super) menu_active: Option<&'static str>,
    pub(super) menu_open: Option<&'static str>,
    pub(super) menu_item_hover: Option<&'static str>,
    pub(super) toolbar_hover: Option<&'static str>,
    pub(super) toolbar_active: Option<&'static str>,
    pub(super) filter_focus: bool,
    pub(super) filter_query: String,
    pub(super) view_mode: &'static str,
    pub(super) modal_dialog: Option<ModalDialogModel>,
    pub(super) modal_hwnd: usize,
    pub(super) modal_dragging: bool,
    pub(super) modal_drag_dx: i32,
    pub(super) modal_drag_dy: i32,
    pub(super) pending_load_tools_request: bool,
    pub(super) status: String,
    pub(super) tree_nodes: Vec<TreeNode>,
    pub(super) preview_path: Option<String>,
    pub(super) preview_name: String,
    pub(super) preview_kind: String,
    pub(super) preview_provider: String,
    pub(super) preview_size: Option<u64>,
    pub(super) preview_type_id: Option<String>,
    pub(super) preview_content_kind: Option<String>,
    pub(super) preview_surface: Option<String>,
    pub(super) preview_lines: Vec<String>,
    pub(super) active_document: Option<TextDocument>,
    pub(super) cached_spans: Vec<TokenSpan>,
    pub(super) xml_path: Option<String>,
    pub(super) xml_lines: Vec<String>,
    pub(super) xml_cursor_line: usize,
    pub(super) xml_cursor_col: usize,
    pub(super) xml_search_query: String,
    pub(super) xml_search_focus: bool,
    pub(super) xml_dirty: bool,
    pub(super) caret_visible: bool,
    pub(super) modal_text_selection_dragging: bool,
    pub(super) modal_text_selection_drag_anchor: Option<usize>,
    pub(super) editor_theme: EditorColorDictionary,
}

pub(super) fn builtin_theme_names() -> &'static [&'static str] {
    &[
        "Visual Studio [TM]",
        "Default",
        "Blue",
        "Monokai",
        "Purple",
        "Twilight",
        "Ocean",
        "Classic",
    ]
}

pub(super) fn cycle_editor_theme(state: &mut UiState) {
    let names = builtin_theme_names();
    let current = state.editor_theme.name;
    let index = names.iter().position(|name| *name == current).unwrap_or(0);
    let next = names[(index + 1) % names.len()];
    state.editor_theme = builtin_editor_color_dictionary(next);
    state.status = format!("Editor theme switched to {next}");
    save_editor_settings_theme(&state.root, next);
}

pub(super) fn editor_settings_path(root: &str) -> PathBuf {
    Path::new(root)
        .join("editor")
        .join("northstar-gui-editor")
        .join("editor_settings.json")
}

pub(super) fn load_editor_settings_theme(root: &str) -> String {
    let path = editor_settings_path(root);
    let Ok(text) = fs::read_to_string(path) else {
        return "Visual Studio [TM]".to_owned();
    };
    extract_json_string(&text, "active_editor_theme")
        .unwrap_or_else(|| "Visual Studio [TM]".to_owned())
}

pub(super) fn save_editor_settings_theme(root: &str, theme: &str) {
    let path = editor_settings_path(root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = format!(
        "{{\n  \"schema\": \"northstar.gui_editor.settings.v1\",\n  \"active_editor_theme\": \"{}\",\n  \"built_in_assets_root\": \"builtIn/assets\"\n}}\n",
        escape_json(theme)
    );
    let _ = fs::write(path, text);
}

pub(super) fn extract_json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)?;
    let tail = &text[start + needle.len()..];
    let colon = tail.find(':')?;
    let tail = tail[colon + 1..].trim_start();
    let tail = tail.strip_prefix('"')?;
    let end = tail.find('"')?;
    Some(tail[..end].to_owned())
}

pub(super) fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Clone, Copy)]
pub(super) struct UiRow {
    pub(super) name: &'static str,
    pub(super) kind: &'static str,
    pub(super) provider: &'static str,
    pub(super) path: &'static str,
    pub(super) size: Option<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct TreeNode {
    pub(super) label: String,
    pub(super) path: String,
    pub(super) indent: usize,
    pub(super) is_package: bool,
    pub(super) has_children: bool,
    pub(super) is_expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoverPanel {
    None,
    Tree,
    List,
    Inspector,
}

#[derive(Debug, Clone)]
pub(super) struct EditorColorDictionary {
    pub(super) name: &'static str,
    pub(super) background: &'static str,
    pub(super) active_line_background: &'static str,
    pub(super) line_numbers: &'static str,
    pub(super) folding_line: &'static str,
    pub(super) search_background: &'static str,
    pub(super) search_foreground: &'static str,
    pub(super) selection_background: &'static str,
    pub(super) editor_foreground: &'static str,
    pub(super) comment: &'static str,
    pub(super) string: &'static str,
    pub(super) reserved_word: &'static str,
    pub(super) symbol: &'static str,
    pub(super) number: &'static str,
    pub(super) attribute: &'static str,
    pub(super) method: &'static str,
}

pub(super) fn builtin_editor_color_dictionary(name: &str) -> EditorColorDictionary {
    match name {
        "Blue" => EditorColorDictionary {
            name: "Blue",
            background: "clWhite",
            active_line_background: "$00E6FAFF",
            line_numbers: "$00CC9999",
            folding_line: "$00CC9999",
            search_background: "$0078AAFF",
            search_foreground: "clBlack",
            selection_background: "$00A56D53",
            editor_foreground: "$00333333",
            comment: "$00999999",
            string: "$005049d4",
            reserved_word: "$009f6f2f",
            symbol: "clNavy",
            number: "$00AAAA33",
            attribute: "$00cf9f4f",
            method: "$009f6f2f",
        },
        "Classic" => EditorColorDictionary {
            name: "Classic",
            background: "clNavy",
            active_line_background: "$009F0000",
            line_numbers: "clSilver",
            folding_line: "clSilver",
            search_background: "clBlack",
            search_foreground: "$00FCFDCD",
            selection_background: "clSilver",
            editor_foreground: "clYellow",
            comment: "clSilver",
            string: "clAqua",
            reserved_word: "clWhite",
            symbol: "clLime",
            number: "clFuchsia",
            attribute: "clYellow",
            method: "clWhite",
        },
        "Default" => EditorColorDictionary {
            name: "Default",
            background: "clWhite",
            active_line_background: "$00E6FAFF",
            line_numbers: "$00CC9999",
            folding_line: "$00CC9999",
            search_background: "$0078AAFF",
            search_foreground: "clBlack",
            selection_background: "$00A56D53",
            editor_foreground: "clBlack",
            comment: "clGreen",
            string: "clBlue",
            reserved_word: "clNavy",
            symbol: "clNavy",
            number: "clBlue",
            attribute: "clMaroon",
            method: "clNavy",
        },
        "Monokai" => EditorColorDictionary {
            name: "Monokai",
            background: "$00222827",
            active_line_background: "$00323D3E",
            line_numbers: "$009F9F9F",
            folding_line: "$00414746",
            search_background: "$003E4849",
            search_foreground: "clNone",
            selection_background: "$003E4849",
            editor_foreground: "$00F8F8F2",
            comment: "$005E7175",
            string: "$0074DBE6",
            reserved_word: "$007226F9",
            symbol: "$00F2F8F8",
            number: "$00FF81AE",
            attribute: "$002AE27F",
            method: "$00EFD966",
        },
        "Ocean" => EditorColorDictionary {
            name: "Ocean",
            background: "clAqua",
            active_line_background: "$00CCFFCC",
            line_numbers: "clTeal",
            folding_line: "clTeal",
            search_background: "$00FCFDCD",
            search_foreground: "clBlack",
            selection_background: "clBlue",
            editor_foreground: "clBlue",
            comment: "clTeal",
            string: "clPurple",
            reserved_word: "clBlack",
            symbol: "clBlack",
            number: "clOlive",
            attribute: "clBlue",
            method: "clBlack",
        },
        "Purple" => EditorColorDictionary {
            name: "Purple",
            background: "clWhite",
            active_line_background: "$00C7EEF8",
            line_numbers: "$00B7B7B7",
            folding_line: "$00B7B7B7",
            search_background: "$0078AAFF",
            search_foreground: "clBlack",
            selection_background: "$00A56D53",
            editor_foreground: "$00333333",
            comment: "$00969896",
            string: "$00913618",
            reserved_word: "$005d1da7",
            symbol: "$00333333",
            number: "$00b38600",
            attribute: "$00913618",
            method: "$005d1da7",
        },
        "Twilight" => EditorColorDictionary {
            name: "Twilight",
            background: "clBlack",
            active_line_background: "$00505050",
            line_numbers: "clLime",
            folding_line: "clLime",
            search_background: "$00FCFDCD",
            search_foreground: "clBlack",
            selection_background: "clBlue",
            editor_foreground: "clWhite",
            comment: "clLime",
            string: "clYellow",
            reserved_word: "clAqua",
            symbol: "clSilver",
            number: "clFuchsia",
            attribute: "clWhite",
            method: "clAqua",
        },
        _ => EditorColorDictionary {
            name: "Visual Studio [TM]",
            background: "clWhite",
            active_line_background: "$00F0FFFF",
            line_numbers: "clGray",
            folding_line: "clGray",
            search_background: "clBlack",
            search_foreground: "clWhite",
            selection_background: "$00FF9933",
            editor_foreground: "clBlack",
            comment: "clGreen",
            string: "clBlue",
            reserved_word: "clBlue",
            symbol: "clBlack",
            number: "clBlue",
            attribute: "clMaroon",
            method: "clBlue",
        },
    }
}
