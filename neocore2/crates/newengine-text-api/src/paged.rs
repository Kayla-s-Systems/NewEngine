use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextPageBreakMode {
    #[default]
    DoubleNewline,
    FormFeed,
    ExplicitToken,
    ParagraphCount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextPageTextRequest {
    pub text: String,
    pub max_pages: usize,
    pub max_chars_per_page: usize,
    pub max_paragraphs_per_page: usize,
    pub break_mode: TextPageBreakMode,
    pub page_token: String,
}

impl Default for TextPageTextRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            max_pages: 4,
            max_chars_per_page: 0,
            max_paragraphs_per_page: 1,
            break_mode: TextPageBreakMode::DoubleNewline,
            page_token: "~PAGE~".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextPageTextResponse {
    pub version: u32,
    pub pages: Vec<String>,
    pub source_chars: usize,
    pub consumed_chars: usize,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
}

impl Default for TextPageTextResponse {
    fn default() -> Self {
        Self {
            version: 1,
            pages: Vec::new(),
            source_chars: 0,
            consumed_chars: 0,
            truncated: false,
            diagnostics: Vec::new(),
        }
    }
}

/// Deterministic utility matching the classic four-page text helper while
/// allowing providers to choose stricter page budgets.
pub fn paginate_text(request: &TextPageTextRequest) -> TextPageTextResponse {
    let normalized = request.text.replace("\r\n", "\n").replace('\r', "\n");
    let max_pages = request.max_pages.max(1);
    let segments = split_segments(&normalized, request);
    let mut pages = Vec::with_capacity(max_pages.min(segments.len()));
    let mut current = String::new();
    let mut paragraphs = 0usize;
    let mut consumed_chars = 0usize;
    let mut truncated = false;

    for segment in segments {
        let separator = if current.is_empty() { "" } else { "\n\n" };
        let candidate_len =
            current.chars().count() + separator.chars().count() + segment.chars().count();
        let chars_full =
            request.max_chars_per_page > 0 && candidate_len > request.max_chars_per_page;
        let paragraphs_full = matches!(request.break_mode, TextPageBreakMode::ParagraphCount)
            && paragraphs >= request.max_paragraphs_per_page.max(1);
        let explicit_page = matches!(
            request.break_mode,
            TextPageBreakMode::DoubleNewline
                | TextPageBreakMode::FormFeed
                | TextPageBreakMode::ExplicitToken
        ) && !current.is_empty();

        if !current.is_empty() && (chars_full || paragraphs_full || explicit_page) {
            pages.push(std::mem::take(&mut current));
            paragraphs = 0;
            if pages.len() >= max_pages {
                truncated = true;
                break;
            }
        }

        if request.max_chars_per_page > 0 && segment.chars().count() > request.max_chars_per_page {
            let mut remainder = segment;
            while !remainder.is_empty() {
                let (chunk, rest) = split_at_char_boundary(remainder, request.max_chars_per_page);
                if !current.is_empty() {
                    pages.push(std::mem::take(&mut current));
                    paragraphs = 0;
                }
                pages.push(chunk.to_owned());
                consumed_chars += chunk.chars().count();
                if pages.len() >= max_pages {
                    truncated = !rest.is_empty();
                    break;
                }
                remainder = rest;
            }
            if truncated || pages.len() >= max_pages {
                break;
            }
            continue;
        }

        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(segment);
        paragraphs += 1;
        consumed_chars += segment.chars().count();
    }

    if !current.is_empty() && pages.len() < max_pages {
        pages.push(current);
    }
    if !truncated {
        consumed_chars = normalized.chars().count();
    }

    TextPageTextResponse {
        version: 1,
        pages,
        source_chars: normalized.chars().count(),
        consumed_chars,
        truncated,
        diagnostics: Vec::new(),
    }
}

fn split_segments<'a>(text: &'a str, request: &TextPageTextRequest) -> Vec<&'a str> {
    let mut segments = match request.break_mode {
        TextPageBreakMode::DoubleNewline | TextPageBreakMode::ParagraphCount => {
            text.split("\n\n").collect::<Vec<_>>()
        }
        TextPageBreakMode::FormFeed => text.split('\u{000c}').collect::<Vec<_>>(),
        TextPageBreakMode::ExplicitToken => text.split(request.page_token.as_str()).collect(),
    };
    segments.retain(|segment| !segment.trim().is_empty());
    for segment in &mut segments {
        *segment = segment.trim();
    }
    segments
}

fn split_at_char_boundary(input: &str, max_chars: usize) -> (&str, &str) {
    if input.chars().count() <= max_chars {
        return (input, "");
    }
    let byte_index = input
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.split_at(byte_index)
}
