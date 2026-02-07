use crate::draw::UiTexId;

pub mod reserved {
    use super::UiTexId;

    pub const FONT_ATLAS: UiTexId = UiTexId(1);
    pub const USER_BEGIN: u32 = 16;
}

#[derive(Debug, Default)]
pub struct UiTexAllocator {
    next: u32,
}

impl UiTexAllocator {
    #[inline]
    pub fn new() -> Self {
        Self {
            next: reserved::USER_BEGIN,
        }
    }

    #[inline]
    pub fn alloc(&mut self) -> UiTexId {
        let id = UiTexId(self.next);
        self.next = self.next.saturating_add(1);
        id
    }
}