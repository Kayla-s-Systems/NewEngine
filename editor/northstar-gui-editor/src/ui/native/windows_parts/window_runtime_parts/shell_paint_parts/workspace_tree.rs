use super::*;

pub(super) unsafe fn draw_workspace_tree(hdc: Hdc, rect: Rect, state: &UiState) {
    let mut y = rect.top + 44;
    for (i, node) in state
        .tree_nodes
        .iter()
        .enumerate()
        .skip(state.tree_scroll_rows)
    {
        if state.selected_tree == i {
            fill(
                hdc,
                Rect {
                    left: rect.left + 6,
                    top: y - 2,
                    right: rect.right - 6,
                    bottom: y + 22,
                },
                rgb(219, 234, 254),
            );
        }
        let indent_px = node.indent as i32 * 24;
        let branch = if node.indent == 0 { "" } else { "└─ " };
        let arrow = if node.has_children {
            if node.is_expanded {
                "▾"
            } else {
                "▸"
            }
        } else {
            " "
        };
        let left = rect.left + 12 + indent_px;
        let prefix = format!("{branch}{arrow}");
        draw_text(
            hdc,
            Rect {
                left,
                top: y,
                right: left + 34,
                bottom: y + 22,
            },
            &prefix,
            rgb(100, 116, 139),
            false,
        );
        draw_builtin_tree_icon(hdc, left + 34, y + 3, node);
        draw_text(
            hdc,
            Rect {
                left: left + 54,
                top: y,
                right: rect.right - 8,
                bottom: y + 22,
            },
            &node.label,
            if node.is_package {
                rgb(112, 74, 18)
            } else {
                rgb(35, 48, 64)
            },
            node.indent == 0 || node.is_package,
        );
        y += 24;
        if y > rect.bottom - 28 {
            break;
        }
    }
}

pub(super) unsafe fn draw_center_surface(hdc: Hdc, rect: Rect, state: &UiState) {
    if state.xml_path.is_some() {
        draw_xml_editor(hdc, rect, state);
    } else if state.preview_path.is_some() {
        draw_asset_preview_editor(hdc, rect, state);
    } else {
        draw_file_table(hdc, rect, state);
    }
}
