use super::{TextBuffer, TextSelection};
use super::search::{SearchMatch, SearchState};

#[derive(Debug, Clone)]
pub struct EditCommand {
    pub start: usize,
    pub removed: String,
    pub inserted: String,
}

#[derive(Debug, Clone)]
pub struct TextDocument {
    pub content_kind: String,
    pub buffer: TextBuffer,
    pub selections: Vec<TextSelection>,
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
}

impl TextDocument {
    pub fn new(content_kind: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            content_kind: content_kind.into(),
            buffer: TextBuffer::new(text),
            selections: vec![TextSelection::caret(0)],
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn set_carets(&mut self, offsets: impl IntoIterator<Item = usize>) {
        let mut selections: Vec<_> = offsets
            .into_iter()
            .map(|offset| TextSelection::caret(offset.min(self.buffer.len())))
            .collect();
        selections.sort_by_key(|selection| selection.cursor);
        selections.dedup_by_key(|selection| selection.cursor);
        if selections.is_empty() {
            selections.push(TextSelection::caret(0));
        }
        self.selections = selections;
    }

    pub fn add_caret(&mut self, offset: usize) {
        let offset = offset.min(self.buffer.len());
        if !self.selections.iter().any(|selection| selection.is_caret() && selection.cursor == offset) {
            self.selections.push(TextSelection::caret(offset));
            self.selections.sort_by_key(|selection| selection.cursor);
        }
    }

    pub fn set_selections(&mut self, selections: impl IntoIterator<Item = TextSelection>) {
        let mut selections: Vec<_> = selections.into_iter().collect();
        selections.sort_by_key(|selection| selection.normalized().0);
        if selections.is_empty() {
            selections.push(TextSelection::caret(0));
        }
        self.selections = selections;
    }

    pub fn insert_text(&mut self, text: &str) {
        self.replace_selections(text);
    }

    pub fn insert_newline(&mut self) {
        self.replace_selections("\n");
    }

    pub fn backspace(&mut self) {
        if self.selections.iter().any(|selection| !selection.is_caret()) {
            self.replace_selections("");
            return;
        }
        let selections = self.selections.clone();
        self.selections = selections
            .into_iter()
            .map(|selection| {
                let cursor = selection.cursor;
                let start = previous_char_boundary(self.buffer.as_str(), cursor);
                TextSelection::range(start, cursor)
            })
            .collect();
        self.replace_selections("");
    }

    pub fn delete_forward(&mut self) {
        if self.selections.iter().any(|selection| !selection.is_caret()) {
            self.replace_selections("");
            return;
        }
        let selections = self.selections.clone();
        self.selections = selections
            .into_iter()
            .map(|selection| {
                let cursor = selection.cursor;
                let end = next_char_boundary(self.buffer.as_str(), cursor);
                TextSelection::range(cursor, end)
            })
            .collect();
        self.replace_selections("");
    }

    pub fn find_all(&self, search: &SearchState) -> Vec<SearchMatch> {
        search.find_all(self.buffer.as_str())
    }

    pub fn select_matches(&mut self, search: &SearchState) -> usize {
        let matches = self.find_all(search);
        self.set_selections(matches.iter().map(|m| TextSelection::range(m.start, m.end)));
        matches.len()
    }

    pub fn replace_all(&mut self, search: &SearchState, replacement: &str) -> usize {
        let matches = self.find_all(search);
        if matches.is_empty() {
            return 0;
        }
        self.set_selections(matches.iter().map(|m| TextSelection::range(m.start, m.end)));
        self.replace_selections(replacement);
        matches.len()
    }

    pub fn replace_selections(&mut self, text: &str) {
        let mut selections = self.selections.clone();
        selections.sort_by_key(|selection| selection.normalized().0);
        let mut new_carets = Vec::new();
        for selection in selections.into_iter().rev() {
            let (start, end) = selection.normalized();
            let removed = self.buffer.replace_range(start, end, text);
            self.undo.push(EditCommand {
                start,
                removed,
                inserted: text.to_owned(),
            });
            new_carets.push(TextSelection::caret(start + text.len()));
        }
        new_carets.reverse();
        self.selections = new_carets;
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(command) = self.undo.pop() else {
            return false;
        };
        let end = command.start + command.inserted.len();
        self.buffer.replace_range(command.start, end, &command.removed);
        self.selections = vec![TextSelection::caret(command.start + command.removed.len())];
        self.redo.push(command);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        let end = command.start + command.removed.len();
        self.buffer.replace_range(command.start, end, &command.inserted);
        self.selections = vec![TextSelection::caret(command.start + command.inserted.len())];
        self.undo.push(command);
        true
    }
}

fn previous_char_boundary(text: &str, offset: usize) -> usize {
    let mut current = offset.min(text.len());
    if current == 0 {
        return 0;
    }
    current -= 1;
    while current > 0 && !text.is_char_boundary(current) {
        current -= 1;
    }
    current
}

fn next_char_boundary(text: &str, offset: usize) -> usize {
    let mut current = offset.min(text.len());
    if current >= text.len() {
        return text.len();
    }
    current += 1;
    while current < text.len() && !text.is_char_boundary(current) {
        current += 1;
    }
    current
}
