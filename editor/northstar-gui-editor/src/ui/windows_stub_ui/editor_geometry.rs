use super::win32::Rect;

#[derive(Debug, Clone, Copy)]
pub struct EditorGeometry {
    pub editor: Rect,
    pub metadata: Rect,
    pub content: Rect,
    pub text_top: i32,
    pub line_left: i32,
    pub gutter_left: i32,
    pub line_height: i32,
    pub char_width: i32,
}

impl EditorGeometry {
    pub fn from_modal_client(client: Rect) -> Self {
        let content = Rect {
            left: 24,
            top: 72,
            right: client.right - 24,
            bottom: client.bottom - 72,
        };
        let metadata = Rect {
            left: content.right - 250,
            top: content.top,
            right: content.right,
            bottom: content.bottom,
        };
        let editor = Rect {
            left: content.left,
            top: content.top,
            right: metadata.left - 18,
            bottom: content.bottom,
        };
        Self {
            editor,
            metadata,
            content,
            text_top: editor.top + 46,
            line_left: editor.left + 66,
            gutter_left: editor.left + 8,
            line_height: 20,
            char_width: 8,
        }
    }

    pub fn scrollbar_rect(&self) -> Rect {
        Rect {
            left: self.editor.right - 14,
            top: self.editor.top + 38,
            right: self.editor.right - 4,
            bottom: self.editor.bottom - 4,
        }
    }

    pub fn visible_lines(&self) -> usize {
        ((self.editor.bottom - self.editor.top - 54).max(20) / self.line_height) as usize
    }

    pub fn line_col_from_point(
        &self,
        x: i32,
        y: i32,
        scroll_rows: usize,
    ) -> Option<(usize, usize)> {
        if x < self.line_left || y < self.text_top {
            return None;
        }
        if x >= self.editor.right || y >= self.editor.bottom {
            return None;
        }
        let visual_line = ((y - self.text_top) / self.line_height).max(0) as usize;
        let line = scroll_rows + visual_line;
        let col = ((x - self.line_left) / self.char_width).max(0) as usize;
        Some((line, col))
    }
}
