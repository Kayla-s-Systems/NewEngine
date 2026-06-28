#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapLine {
    pub line: usize,
    pub indentation: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinimapModel {
    pub lines: Vec<MinimapLine>,
}

impl MinimapModel {
    pub fn from_text(text: &str) -> Self {
        let lines = text
            .lines()
            .enumerate()
            .map(|(line, content)| MinimapLine {
                line,
                indentation: content.chars().take_while(|ch| ch.is_whitespace()).count(),
                length: content.len(),
            })
            .collect();

        Self { lines }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}
