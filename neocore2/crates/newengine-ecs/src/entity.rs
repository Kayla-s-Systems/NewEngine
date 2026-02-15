#![forbid(unsafe_op_in_unsafe_fn)]

use slotmap::new_key_type;

new_key_type! {
    /// Generational entity identifier.
    pub struct EntityId;
}