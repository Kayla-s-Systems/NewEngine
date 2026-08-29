use std::collections::VecDeque;

use newengine_console_api::CommandSuggestResponse;

pub(super) const OUTPUT_CAPACITY: usize = 256;
pub(super) const HISTORY_CAPACITY: usize = 128;
pub(super) const INPUT_CAPACITY_CHARS: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConsoleLineKind {
    Info,
    Command,
    Output,
    Error,
}

#[derive(Clone, Debug)]
pub(super) struct ConsoleLine {
    pub kind: ConsoleLineKind,
    pub text: String,
}

#[derive(Debug)]
pub(super) struct RuntimeConsoleOverlayState {
    pub open: bool,
    pub buffer: String,
    pub cursor_chars: usize,
    pub history: Vec<String>,
    pub history_cursor: Option<usize>,
    pub output: VecDeque<ConsoleLine>,
    /// Provider-neutral history viewport: 0 follows the newest line; positive
    /// values keep the viewport that many logical lines behind the tail.
    pub output_scroll_lines_from_bottom: usize,
    pub output_scroll_remainder_px: f32,
    pub suggestions: CommandSuggestResponse,
    pub revision: u64,
    pub published_revision: u64,
    pub last_published_surface_size: [u32; 2],
    pub surface_published: bool,
    pub last_suggest_input: String,
}

impl Default for RuntimeConsoleOverlayState {
    fn default() -> Self {
        let mut state = Self {
            open: false,
            buffer: String::new(),
            cursor_chars: 0,
            history: Vec::new(),
            history_cursor: None,
            output: VecDeque::new(),
            output_scroll_lines_from_bottom: 0,
            output_scroll_remainder_px: 0.0,
            suggestions: CommandSuggestResponse::default(),
            revision: 1,
            published_revision: 0,
            last_published_surface_size: [0, 0],
            surface_published: false,
            last_suggest_input: String::new(),
        };
        state.push_line(
            ConsoleLineKind::Info,
            "North Star Runtime Console | ~ toggle | Tab autocomplete | Up/Down history",
        );
        state
    }
}

impl RuntimeConsoleOverlayState {
    #[inline]
    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
    }

    pub fn push_line(&mut self, kind: ConsoleLineKind, text: impl AsRef<str>) {
        let was_scrolled_back = self.output_scroll_lines_from_bottom > 0;
        let mut appended = 0usize;
        for line in text.as_ref().lines() {
            if self.output.len() >= OUTPUT_CAPACITY {
                self.output.pop_front();
            }
            self.output.push_back(ConsoleLine {
                kind,
                text: line.to_owned(),
            });
            appended = appended.saturating_add(1);
        }
        // If the user is inspecting older history, keep that logical viewport stable
        // as new output arrives. Tail-follow remains exactly zero otherwise.
        if was_scrolled_back && appended > 0 {
            self.output_scroll_lines_from_bottom = self
                .output_scroll_lines_from_bottom
                .saturating_add(appended)
                .min(self.output.len().saturating_sub(1));
        }
        self.touch();
    }

    pub fn scroll_output_wheel(&mut self, delta_y_px: f32) -> bool {
        if !delta_y_px.is_finite() || delta_y_px.abs() <= f32::EPSILON || self.output.len() <= 1 {
            return false;
        }
        const PIXELS_PER_LOGICAL_LINE: f32 = 21.0;
        self.output_scroll_remainder_px += delta_y_px;
        let lines = (self.output_scroll_remainder_px / PIXELS_PER_LOGICAL_LINE).trunc() as isize;
        if lines == 0 {
            return false;
        }
        self.output_scroll_remainder_px -= lines as f32 * PIXELS_PER_LOGICAL_LINE;
        let max_back = self.output.len().saturating_sub(1);
        let before = self.output_scroll_lines_from_bottom;
        if lines > 0 {
            self.output_scroll_lines_from_bottom = self
                .output_scroll_lines_from_bottom
                .saturating_add(lines as usize)
                .min(max_back);
        } else {
            self.output_scroll_lines_from_bottom = self
                .output_scroll_lines_from_bottom
                .saturating_sub(lines.unsigned_abs());
        }
        if self.output_scroll_lines_from_bottom != before {
            self.touch();
            true
        } else {
            false
        }
    }

    pub fn follow_output_tail(&mut self) {
        if self.output_scroll_lines_from_bottom != 0 || self.output_scroll_remainder_px != 0.0 {
            self.output_scroll_lines_from_bottom = 0;
            self.output_scroll_remainder_px = 0.0;
            self.touch();
        }
    }

    pub fn remember_command(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        if self.history.last().is_none_or(|last| last != line) {
            if self.history.len() >= HISTORY_CAPACITY {
                self.history.remove(0);
            }
            self.history.push(line.to_owned());
        }
        self.history_cursor = None;
    }

    pub fn set_buffer(&mut self, value: String) {
        let value = value.chars().take(INPUT_CAPACITY_CHARS).collect::<String>();
        self.cursor_chars = value.chars().count();
        self.buffer = value;
        self.history_cursor = None;
        self.touch();
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() || self.buffer.chars().count() >= INPUT_CAPACITY_CHARS {
            return;
        }
        let remaining = INPUT_CAPACITY_CHARS.saturating_sub(self.buffer.chars().count());
        let text = text.chars().take(remaining).collect::<String>();
        let byte = byte_index_for_char(&self.buffer, self.cursor_chars);
        self.buffer.insert_str(byte, &text);
        self.cursor_chars += text.chars().count();
        self.history_cursor = None;
        self.touch();
    }

    pub fn backspace(&mut self) {
        if self.cursor_chars == 0 {
            return;
        }
        let end = byte_index_for_char(&self.buffer, self.cursor_chars);
        let start = byte_index_for_char(&self.buffer, self.cursor_chars - 1);
        self.buffer.replace_range(start..end, "");
        self.cursor_chars -= 1;
        self.touch();
    }

    pub fn delete(&mut self) {
        let count = self.buffer.chars().count();
        if self.cursor_chars >= count {
            return;
        }
        let start = byte_index_for_char(&self.buffer, self.cursor_chars);
        let end = byte_index_for_char(&self.buffer, self.cursor_chars + 1);
        self.buffer.replace_range(start..end, "");
        self.touch();
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let count = self.buffer.chars().count() as isize;
        let next = (self.cursor_chars as isize + delta).clamp(0, count) as usize;
        if next != self.cursor_chars {
            self.cursor_chars = next;
            self.touch();
        }
    }

    pub fn history_step(&mut self, direction: isize) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        let next = match self.history_cursor {
            None if direction < 0 => last,
            None => return,
            Some(index) => (index as isize + direction).clamp(0, last as isize) as usize,
        };
        self.history_cursor = Some(next);
        let value = self.history[next].clone();
        self.buffer = value;
        self.cursor_chars = self.buffer.chars().count();
        self.touch();
    }

    pub fn prompt_with_cursor(&self) -> String {
        let byte = byte_index_for_char(&self.buffer, self.cursor_chars);
        let (left, right) = self.buffer.split_at(byte);
        format!("> {left}▌{right}")
    }
}

#[inline]
fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_cursor_edits_are_char_safe() {
        let mut state = RuntimeConsoleOverlayState::default();
        state.set_buffer("set тест 1".to_owned());
        state.move_cursor(-1);
        state.backspace();
        assert!(state.buffer.is_char_boundary(state.buffer.len()));
        state.insert_text("я");
        assert!(state.buffer.contains('я'));
    }

    #[test]
    fn output_and_history_are_bounded() {
        let mut state = RuntimeConsoleOverlayState::default();
        for index in 0..(OUTPUT_CAPACITY + 20) {
            state.push_line(ConsoleLineKind::Output, format!("line {index}"));
        }
        assert_eq!(state.output.len(), OUTPUT_CAPACITY);
        for index in 0..(HISTORY_CAPACITY + 20) {
            state.remember_command(&format!("cmd {index}"));
        }
        assert_eq!(state.history.len(), HISTORY_CAPACITY);
    }
}
