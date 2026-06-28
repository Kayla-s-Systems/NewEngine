use super::*;

pub(super) unsafe fn draw_file_table(hdc: Hdc, rect: Rect, state: &UiState) {
    let header_y = rect.top + 32;
    fill(
        hdc,
        Rect {
            left: rect.left,
            top: header_y,
            right: rect.right,
            bottom: header_y + 28,
        },
        rgb(250, 251, 252),
    );
    line_frame(
        hdc,
        Rect {
            left: rect.left,
            top: header_y + 27,
            right: rect.right,
            bottom: header_y + 28,
        },
        rgb(222, 226, 232),
    );
    let c1 = rect.left + 16;
    let c2 = rect.left + 330;
    let c3 = rect.left + 505;
    draw_text(
        hdc,
        Rect {
            left: c1,
            top: header_y + 7,
            right: c2 - 12,
            bottom: header_y + 25,
        },
        "Name",
        rgb(75, 85, 99),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: c2,
            top: header_y + 7,
            right: c3 - 12,
            bottom: header_y + 25,
        },
        "Type",
        rgb(75, 85, 99),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: c3,
            top: header_y + 7,
            right: rect.right - 140,
            bottom: header_y + 25,
        },
        "Provider / Route",
        rgb(75, 85, 99),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: rect.right - 132,
            top: header_y + 7,
            right: rect.right - 16,
            bottom: header_y + 25,
        },
        &format!("View: {}", state.view_mode),
        rgb(37, 99, 235),
        true,
    );

    let rows = sample_rows();
    let mut y = row_start(rect);
    for (visible, row) in rows.iter().enumerate().skip(state.scroll_rows) {
        if visible == state.selected_row {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y - 3,
                    right: rect.right - 1,
                    bottom: y + 22,
                },
                rgb(37, 99, 235),
            );
        } else if state.hover_row == Some(visible) {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y - 3,
                    right: rect.right - 1,
                    bottom: y + 22,
                },
                rgb(226, 232, 240),
            );
        } else if visible % 2 == 1 {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y - 3,
                    right: rect.right - 1,
                    bottom: y + 22,
                },
                rgb(248, 250, 252),
            );
        }
        let color = if visible == state.selected_row {
            rgb(255, 255, 255)
        } else {
            rgb(31, 41, 55)
        };
        draw_builtin_row_icon(hdc, c1, y + 3, row, visible == state.selected_row);
        draw_text(
            hdc,
            Rect {
                left: c1 + 24,
                top: y,
                right: c2 - 12,
                bottom: y + 22,
            },
            row.name,
            color,
            false,
        );
        draw_text(
            hdc,
            Rect {
                left: c2,
                top: y,
                right: c3 - 12,
                bottom: y + 22,
            },
            row.kind,
            color,
            false,
        );
        draw_text(
            hdc,
            Rect {
                left: c3,
                top: y,
                right: rect.right - 16,
                bottom: y + 22,
            },
            row.provider,
            color,
            false,
        );
        y += row_height();
        if y > rect.bottom - 30 {
            break;
        }
    }
}
