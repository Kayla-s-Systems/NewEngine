use super::*;

pub(super) fn is_package_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "rpf" | "nepak" | "pak" | "zip"
            )
        })
        .unwrap_or(false)
}

pub(super) fn folder_rank(kind: &str) -> u8 {
    match kind {
        "Folder" => 2,
        "Package" => 1,
        _ => 0,
    }
}

pub(super) fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

pub(super) fn format_size(size: Option<u64>) -> String {
    match size {
        None => "".to_owned(),
        Some(value) if value < 1024 => format!("{value} B"),
        Some(value) if value < 1024 * 1024 => format!("{} KB", value / 1024),
        Some(value) => format!("{} MB", value / (1024 * 1024)),
    }
}

pub(super) fn hit_menu(x: i32, y: i32) -> Option<&'static str> {
    if !(0..=31).contains(&y) {
        return None;
    }
    for item in menu_model::top_menu_items() {
        let width = menu_model::top_menu_item_width(item.label);
        if x >= item.x && x <= item.x + width {
            return Some(item.label);
        }
    }
    None
}

pub(super) unsafe fn draw_menu_item(hdc: Hdc, x: i32, label: &str, hovered: bool, active: bool) {
    let width = menu_model::top_menu_item_width(label);
    let rect = Rect {
        left: x,
        top: 3,
        right: x + width,
        bottom: 29,
    };
    if active {
        fill(hdc, rect, rgb(219, 234, 254));
        line_frame(hdc, rect, rgb(37, 99, 235));
    } else if hovered {
        fill(hdc, rect, rgb(239, 246, 255));
        line_frame(hdc, rect, rgb(147, 197, 253));
    }
    draw_text(
        hdc,
        Rect {
            left: x + 7,
            top: 8,
            right: x + width - 6,
            bottom: 27,
        },
        label,
        if active {
            rgb(30, 64, 175)
        } else {
            rgb(32, 42, 54)
        },
        active,
    );
}

pub(super) fn menu_dropdown_rect(menu: &'static str) -> Option<Rect> {
    let item = menu_model::top_menu_items()
        .iter()
        .copied()
        .find(|item| item.label == menu)?;
    let width = menu_model::dropdown_width(menu);
    let height = menu_model::dropdown_height(menu);
    Some(Rect {
        left: item.x,
        top: 31,
        right: item.x + width,
        bottom: 31 + height,
    })
}

pub(super) fn hit_menu_dropdown(
    x: i32,
    y: i32,
    open_menu: Option<&'static str>,
) -> Option<(&'static str, &'static str)> {
    let menu = open_menu?;
    let rect = menu_dropdown_rect(menu)?;
    if !point_in(rect, x, y) {
        return None;
    }
    let mut y_cursor = rect.top + 4;
    for item in menu_model::dropdown_items(menu) {
        if menu_model::is_separator(item) {
            y_cursor += 8;
            continue;
        }
        if y >= y_cursor && y < y_cursor + 24 {
            return Some((menu, item));
        }
        y_cursor += 24;
    }
    None
}

pub(super) fn make_layout(client: Rect) -> Layout {
    let status_h = 30;
    let top = 78;
    let left_w = 292;
    let right_w = 322;
    Layout {
        left: Rect {
            left: 0,
            top,
            right: left_w,
            bottom: client.bottom - status_h,
        },
        center: Rect {
            left: left_w,
            top,
            right: client.right - right_w,
            bottom: client.bottom - status_h,
        },
        right: Rect {
            left: client.right - right_w,
            top,
            right: client.right,
            bottom: client.bottom - status_h,
        },
        status: Rect {
            left: 0,
            top: client.bottom - status_h,
            right: client.right,
            bottom: client.bottom,
        },
    }
}

pub(super) fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

pub(super) fn sample_rows() -> Vec<UiRow> {
    let Some(state) = cloned_state() else {
        return Vec::new();
    };
    let path = Path::new(&state.selected_path);
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new(&state.root))
            .to_path_buf()
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for entry in entries.flatten().take(512) {
        let entry_path = entry.path();
        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = if is_dir {
            "Folder".to_owned()
        } else {
            entry_path
                .extension()
                .and_then(|v| v.to_str())
                .map(|ext| format!(".{ext}"))
                .unwrap_or_else(|| "File".to_owned())
        };
        let provider = if is_dir {
            "filesystem".to_owned()
        } else {
            preview_provider_for_extension(&entry_path)
        };
        rows.push(UiRow {
            name: leak_string(name),
            kind: leak_string(kind),
            provider: leak_string(provider),
            path: leak_string(entry_path.display().to_string()),
            size: metadata.map(|m| m.len()),
        });
    }
    rows.sort_by_key(|row| (row.kind != "Folder", row.name.to_ascii_lowercase()));
    rows
}

pub(super) fn preview_provider_for_extension(path: &Path) -> String {
    let ext = normalized_extension(path);
    if ext.is_empty() {
        "northstar.editor.filesystem".to_owned()
    } else {
        format!("northstar.editor{ext}.builtin")
    }
}

pub(super) fn load_tree_nodes(root: &Path, expanded_paths: &[String]) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    load_tree_nodes_inner(root, 0, expanded_paths, &mut nodes, 512);
    nodes
}

pub(super) fn load_tree_nodes_inner(
    path: &Path,
    indent: usize,
    expanded_paths: &[String],
    nodes: &mut Vec<TreeNode>,
    limit: usize,
) {
    if nodes.len() >= limit {
        return;
    }
    let path_text = path.display().to_string();
    let label = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("Workspace"))
        .to_owned();
    let has_children = path.is_dir()
        && fs::read_dir(path)
            .map(|mut it| it.any(|e| e.ok().is_some_and(|v| v.path().is_dir())))
            .unwrap_or(false);
    let is_expanded = expanded_paths.iter().any(|item| item == &path_text);
    nodes.push(TreeNode {
        label,
        path: path_text.clone(),
        indent,
        is_package: is_package_path(path),
        has_children,
        is_expanded,
    });
    if !is_expanded {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort_by_key(|p| {
        p.file_name()
            .map(|v| v.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    for dir in dirs.into_iter().take(64) {
        load_tree_nodes_inner(&dir, indent + 1, expanded_paths, nodes, limit);
        if nodes.len() >= limit {
            break;
        }
    }
}

pub(super) fn toggle_expanded_path(expanded_paths: &mut Vec<String>, path: &str) {
    if let Some(index) = expanded_paths.iter().position(|item| item == path) {
        expanded_paths.remove(index);
    } else {
        expanded_paths.push(path.to_owned());
    }
}

pub(super) fn find_tree_index(nodes: &[TreeNode], path: &str) -> Option<usize> {
    nodes.iter().position(|node| node.path == path)
}

pub(super) fn validate_selected_asset(state: &mut UiState, rows: &[UiRow]) {
    let Some(item) = rows.get(state.selected_row).or_else(|| rows.first()) else {
        state.modal_dialog = Some(ModalDialogModel::message(
            "Validate",
            "Nothing is selected for validation.",
        ));
        state.status = "Validate: nothing selected".to_owned();
        return;
    };
    let exists = Path::new(item.path).exists();
    state.modal_dialog = Some(ModalDialogModel::message(
        "Validate",
        format!(
            "{}\nType: {}\nProvider: {}\nSize: {}\nExists: {}",
            item.name,
            item.kind,
            item.provider,
            format_size(item.size),
            exists,
        ),
    ));
    state.status = if exists {
        format!("Validated: {}", item.name)
    } else {
        format!("Validate failed: missing {}", item.name)
    };
}
