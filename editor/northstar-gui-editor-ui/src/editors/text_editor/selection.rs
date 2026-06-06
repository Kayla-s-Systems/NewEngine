#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelection { pub anchor: usize, pub cursor: usize }
impl TextSelection {
    pub fn caret(offset: usize) -> Self { Self { anchor: offset, cursor: offset } }
    pub fn range(anchor: usize, cursor: usize) -> Self { Self { anchor, cursor } }
    pub fn is_caret(&self) -> bool { self.anchor == self.cursor }
    pub fn normalized(&self) -> (usize, usize) { if self.anchor <= self.cursor { (self.anchor, self.cursor) } else { (self.cursor, self.anchor) } }
}
