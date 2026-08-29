use newengine_scripting_api::{
    ScriptingCompletionItem, ScriptingCompletionRequest, ScriptingCompletionResponse,
};

use crate::ScriptingToolingClient;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompletionPopupState {
    pub visible: bool,
    pub selected_index: Option<usize>,
    pub items: Vec<ScriptingCompletionItem>,
    pub provider: String,
    pub diagnostics: Vec<String>,
    pub replacement_start_byte: usize,
    pub replacement_end_byte: usize,
}

impl CompletionPopupState {
    #[inline]
    pub fn selected_item(&self) -> Option<&ScriptingCompletionItem> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(index) => (index + 1) % self.items.len(),
            None => 0,
        });
    }

    pub fn select_previous(&mut self) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(0) | None => self.items.len() - 1,
            Some(index) => index - 1,
        });
    }

    #[inline]
    pub fn dismiss(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptCodeEditorSession {
    module_ref: String,
    language_id: String,
    source_text: String,
    cursor_byte_offset: usize,
    document_revision: u64,
    popup: CompletionPopupState,
}

impl ScriptCodeEditorSession {
    pub fn new(
        module_ref: impl Into<String>,
        language_id: impl Into<String>,
        source_text: impl Into<String>,
    ) -> Self {
        let source_text = source_text.into();
        let cursor_byte_offset = source_text.len();
        Self {
            module_ref: module_ref.into(),
            language_id: language_id.into(),
            source_text,
            cursor_byte_offset,
            document_revision: 1,
            popup: CompletionPopupState::default(),
        }
    }

    #[inline]
    pub fn module_ref(&self) -> &str {
        &self.module_ref
    }

    #[inline]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    #[inline]
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    #[inline]
    pub fn cursor_byte_offset(&self) -> usize {
        self.cursor_byte_offset
    }

    #[inline]
    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    #[inline]
    pub fn popup(&self) -> &CompletionPopupState {
        &self.popup
    }

    #[inline]
    pub fn popup_mut(&mut self) -> &mut CompletionPopupState {
        &mut self.popup
    }

    pub fn set_cursor_byte_offset(&mut self, cursor: usize) -> Result<(), String> {
        validate_boundary(&self.source_text, cursor)?;
        if self.cursor_byte_offset != cursor {
            self.cursor_byte_offset = cursor;
            // A response for the previous caret position must not be shown at the new caret.
            self.popup.dismiss();
        }
        Ok(())
    }

    pub fn set_source_text(&mut self, source_text: impl Into<String>) {
        self.source_text = source_text.into();
        self.cursor_byte_offset = self.source_text.len();
        self.bump_revision();
    }

    pub fn replace_range(
        &mut self,
        start_byte: usize,
        end_byte: usize,
        replacement: &str,
    ) -> Result<(), String> {
        validate_range(&self.source_text, start_byte, end_byte)?;
        self.source_text
            .replace_range(start_byte..end_byte, replacement);
        self.cursor_byte_offset = start_byte + replacement.len();
        self.bump_revision();
        Ok(())
    }

    #[inline]
    pub fn insert_text(&mut self, text: &str) -> Result<(), String> {
        let cursor = self.cursor_byte_offset;
        self.replace_range(cursor, cursor, text)
    }

    pub fn completion_request(
        &self,
        trigger_character: Option<String>,
        max_items: usize,
    ) -> ScriptingCompletionRequest {
        ScriptingCompletionRequest {
            module_ref: self.module_ref.clone(),
            language_id: self.language_id.clone(),
            source_text: self.source_text.clone(),
            document_revision: self.document_revision,
            cursor_byte_offset: self.cursor_byte_offset,
            trigger_character,
            max_items,
            ..ScriptingCompletionRequest::default()
        }
    }

    pub fn refresh_completion(
        &mut self,
        client: &ScriptingToolingClient,
        trigger_character: Option<String>,
        max_items: usize,
    ) -> Result<bool, String> {
        let request = self.completion_request(trigger_character, max_items);
        let response = client.complete(&request)?;
        Ok(self.accept_completion_response(response))
    }

    /// Accept a provider response only when it still belongs to the current document/caret.
    /// This is intentionally safe for asynchronous editor integrations.
    pub fn accept_completion_response(&mut self, response: ScriptingCompletionResponse) -> bool {
        if response.document_revision != self.document_revision
            || !response.language_id.eq_ignore_ascii_case(&self.language_id)
            || response.replacement_end_byte != self.cursor_byte_offset
            || validate_range(
                &self.source_text,
                response.replacement_start_byte,
                response.replacement_end_byte,
            )
            .is_err()
        {
            return false;
        }

        let items = response
            .items
            .into_iter()
            .filter(|item| {
                validate_range(
                    &self.source_text,
                    item.replacement_start_byte,
                    item.replacement_end_byte,
                )
                .is_ok()
                    && item.replacement_end_byte == self.cursor_byte_offset
            })
            .collect::<Vec<_>>();
        let selected_index = (!items.is_empty()).then_some(0);
        self.popup = CompletionPopupState {
            visible: !items.is_empty(),
            selected_index,
            items,
            provider: response.provider,
            diagnostics: response.diagnostics,
            replacement_start_byte: response.replacement_start_byte,
            replacement_end_byte: response.replacement_end_byte,
        };
        true
    }

    pub fn apply_selected_completion(&mut self) -> Result<bool, String> {
        let Some(item) = self.popup.selected_item().cloned() else {
            return Ok(false);
        };
        self.replace_range(
            item.replacement_start_byte,
            item.replacement_end_byte,
            &item.insert_text,
        )?;
        Ok(true)
    }

    #[inline]
    pub fn dismiss_completion(&mut self) {
        self.popup.dismiss();
    }

    fn bump_revision(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1).max(1);
        self.popup.dismiss();
    }
}

fn validate_boundary(source: &str, offset: usize) -> Result<(), String> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return Err(format!(
            "byte offset {offset} is not a UTF-8 boundary for source length {}",
            source.len()
        ));
    }
    Ok(())
}

fn validate_range(source: &str, start: usize, end: usize) -> Result<(), String> {
    validate_boundary(source, start)?;
    validate_boundary(source, end)?;
    if start > end {
        return Err(format!("invalid editor byte range {start}..{end}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion_item(label: &str, start: usize, end: usize) -> ScriptingCompletionItem {
        ScriptingCompletionItem {
            label: label.to_owned(),
            insert_text: label.to_owned(),
            kind: "keyword".to_owned(),
            replacement_start_byte: start,
            replacement_end_byte: end,
            ..ScriptingCompletionItem::default()
        }
    }

    fn response(
        session: &ScriptCodeEditorSession,
        item: ScriptingCompletionItem,
    ) -> ScriptingCompletionResponse {
        ScriptingCompletionResponse {
            provider: "test.provider".to_owned(),
            language_id: session.language_id().to_owned(),
            document_revision: session.document_revision(),
            replacement_start_byte: item.replacement_start_byte,
            replacement_end_byte: item.replacement_end_byte,
            items: vec![item],
            ..ScriptingCompletionResponse::default()
        }
    }

    #[test]
    fn completion_request_tracks_document_revision_and_caret() {
        let mut session = ScriptCodeEditorSession::new("scripts/test.ysc", "typescript", "ret");
        session.set_cursor_byte_offset(3).unwrap();
        let request = session.completion_request(None, 64);
        assert_eq!(request.document_revision, session.document_revision());
        assert_eq!(request.cursor_byte_offset, 3);
        assert_eq!(request.source_text, "ret");
    }

    #[test]
    fn stale_revision_response_is_rejected() {
        let mut session = ScriptCodeEditorSession::new("scripts/test.ysc", "typescript", "ret");
        let mut stale = response(&session, completion_item("return", 0, 3));
        session.insert_text("x").unwrap();
        stale.replacement_end_byte = session.cursor_byte_offset();
        assert!(!session.accept_completion_response(stale));
        assert!(!session.popup().visible);
    }

    #[test]
    fn caret_move_rejects_response_for_previous_position() {
        let mut session = ScriptCodeEditorSession::new("scripts/test.ysc", "typescript", "ret x");
        session.set_cursor_byte_offset(3).unwrap();
        let pending = response(&session, completion_item("return", 0, 3));
        session.set_cursor_byte_offset(5).unwrap();
        assert!(!session.accept_completion_response(pending));
    }

    #[test]
    fn applying_selected_completion_replaces_provider_range_and_bumps_revision() {
        let mut session = ScriptCodeEditorSession::new("scripts/test.ysc", "typescript", "ret");
        let revision = session.document_revision();
        let response = response(&session, completion_item("return", 0, 3));
        assert!(session.accept_completion_response(response));
        assert!(session.popup().visible);
        assert!(session.apply_selected_completion().unwrap());
        assert_eq!(session.source_text(), "return");
        assert_eq!(session.cursor_byte_offset(), 6);
        assert_ne!(session.document_revision(), revision);
        assert!(!session.popup().visible);
    }

    #[test]
    fn popup_selection_wraps_in_both_directions() {
        let mut popup = CompletionPopupState {
            visible: true,
            selected_index: Some(0),
            items: vec![completion_item("one", 0, 0), completion_item("two", 0, 0)],
            ..CompletionPopupState::default()
        };
        popup.select_previous();
        assert_eq!(popup.selected_index, Some(1));
        popup.select_next();
        assert_eq!(popup.selected_index, Some(0));
    }
}
