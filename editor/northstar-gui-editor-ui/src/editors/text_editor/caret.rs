#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret { pub offset: usize }
impl Caret { pub fn new(offset: usize) -> Self { Self { offset } } }
