#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    pub start: usize,
    pub end: usize,
}

impl SearchState {
    pub fn find_all(&self, text: &str) -> Vec<SearchMatch> {
        if self.query.is_empty() {
            return Vec::new();
        }

        let haystack = if self.case_sensitive {
            text.to_owned()
        } else {
            text.to_ascii_lowercase()
        };
        let needle = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_ascii_lowercase()
        };

        let mut matches = Vec::new();
        let mut offset = 0;
        while let Some(found) = haystack[offset..].find(&needle) {
            let start = offset + found;
            let end = start + needle.len();
            matches.push(SearchMatch { start, end });
            offset = end.max(start + 1);
            if offset >= haystack.len() {
                break;
            }
        }
        matches
    }
}
