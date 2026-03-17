use crate::draw::UiTexId;

pub mod reserved {
    pub use newengine_ui_draw::reserved::*;
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
