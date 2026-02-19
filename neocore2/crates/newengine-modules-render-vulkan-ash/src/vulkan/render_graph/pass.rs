#![forbid(unsafe_code)]

use std::fmt;

use super::resource::{ResourceId, ResourceKind};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassId(pub u32);

impl fmt::Debug for PassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassRead {
    pub res: ResourceId,
    pub kind: ResourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassWrite {
    pub res: ResourceId,
    pub kind: ResourceKind,
}

/// Declarative pass node: what it reads/writes.
///
/// Execution function pointers are intentionally not included here.
/// The renderer backend can map `PassId` to an executable record at runtime.
#[derive(Clone, Debug)]
pub struct PassNode {
    pub id: PassId,
    pub name: &'static str,
    pub reads: Vec<PassRead>,
    pub writes: Vec<PassWrite>,
}
