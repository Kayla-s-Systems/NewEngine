use crate::draw::UiTexId;

pub mod reserved {
    use super::UiTexId;

    pub const FONT_ATLAS: UiTexId = UiTexId(1);
    pub const USER_BEGIN: u32 = 16;

    /// Reserved range for external (GPU-owned) textures.
    ///
    /// Contract:
    /// - `egui::TextureId::User(u64)` is passed through as `UiTexId(u32)`.
    /// - External textures must not collide with engine-managed ids.
    /// - The high bit is used as a namespace fence.
    pub const EXTERNAL_BEGIN: u32 = 0x8000_0000;

    #[inline]
    pub const fn external_from_u32(local: u32) -> UiTexId {
        UiTexId(EXTERNAL_BEGIN | (local & 0x7FFF_FFFF))
    }

    #[inline]
    pub const fn is_external(id: UiTexId) -> bool {
        (id.0 & EXTERNAL_BEGIN) != 0
    }
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