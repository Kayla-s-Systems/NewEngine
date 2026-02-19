#![forbid(unsafe_code)]

use std::fmt;

/// Opaque resource handle used inside a render graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u32);

impl fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Buffer,
    Image,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphBufferDesc {
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphImageDesc {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mips: u32,
    /// Backend-specific format token (Vulkan uses VkFormat). Stored as u32 to keep this module pure.
    pub format: u32,
}
