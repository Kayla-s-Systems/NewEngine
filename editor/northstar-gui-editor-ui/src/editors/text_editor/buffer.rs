#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextBuffer {
    text: String,
    line_starts: Vec<usize>,
}

impl TextBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let mut buffer = Self { text: text.into(), line_starts: Vec::new() };
        buffer.rebuild_line_index();
        buffer
    }

    pub fn as_str(&self) -> &str { &self.text }
    pub fn len(&self) -> usize { self.text.len() }
    pub fn is_empty(&self) -> bool { self.text.is_empty() }
    pub fn line_count(&self) -> usize { self.line_starts.len().max(1) }

    pub fn line(&self, line: usize) -> Option<&str> {
        let start = *self.line_starts.get(line)?;
        let end = self.line_starts.get(line + 1).copied().unwrap_or_else(|| self.text.len());
        Some(self.text[start..end].trim_end_matches(['\r', '\n']))
    }

    pub fn offset_for_line_column(&self, line: usize, column: usize) -> usize {
        let Some(&line_start) = self.line_starts.get(line) else { return self.text.len(); };
        let line_end = self.line_starts.get(line + 1).copied().unwrap_or_else(|| self.text.len());
        let max = self.text[line_start..line_end].trim_end_matches(['\r', '\n']).len();
        let offset = line_start + column.min(max);
        clamp_to_char_boundary(&self.text, offset)
    }

    pub fn line_column_for_offset(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line = match self.line_starts.binary_search(&offset) { Ok(index) => index, Err(0) => 0, Err(index) => index - 1 };
        let column = offset.saturating_sub(self.line_starts.get(line).copied().unwrap_or(0));
        (line, column)
    }

    pub fn replace_range(&mut self, start: usize, end: usize, replacement: &str) -> String {
        let start = clamp_to_char_boundary(&self.text, start.min(self.text.len()));
        let end = clamp_to_char_boundary(&self.text, end.min(self.text.len()).max(start));
        let removed = self.text[start..end].to_owned();
        self.text.replace_range(start..end, replacement);
        self.rebuild_line_index();
        removed
    }

    fn rebuild_line_index(&mut self) {
        self.line_starts.clear();
        self.line_starts.push(0);
        for (index, byte) in self.text.bytes().enumerate() {
            if byte == b'\n' { self.line_starts.push(index + 1); }
        }
    }
}

fn clamp_to_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) { offset -= 1; }
    offset
}
