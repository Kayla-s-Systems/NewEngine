//! Escape hatch to raw container implementations.
//!
//! ⚠️ NOT the normal API.
//! Downstream crates MUST prefer `collections::prelude::*` (Ne* aliases).
//! Use raw only when you truly need implementation-specific APIs such as Entry/Iter.

pub use hashbrown;
pub use slotmap;

// Narrow re-exports for common "advanced" APIs:
pub use hashbrown::hash_map;
pub use hashbrown::hash_set;
