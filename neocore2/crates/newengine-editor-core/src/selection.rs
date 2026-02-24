#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::collections_prelude::NeKey;

/// Deterministic selection model used by editor tools and UI.
///
/// Guarantees:
/// - Stable iteration order (sorted by `EntityId` stable key).
/// - Primary selection is always either `None` or an element of `selected`.
#[derive(Debug, Default, Clone)]
pub struct SelectionModel {
    primary: Option<EntityId>,
    selected: Vec<EntityId>,
}

impl SelectionModel {
    #[inline]
    pub fn primary(&self) -> Option<EntityId> {
        self.primary
    }

    /// Returns number of selected entities.
    #[inline]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Deterministic iterator over the selection.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item=EntityId> + '_ {
        self.selected.iter().copied()
    }

    #[inline]
    pub fn set_primary(&mut self, id: Option<EntityId>) {
        self.primary = id;
        if let Some(e) = id {
            if !self.contains(e) {
                self.selected.clear();
                self.selected.push(e);
            }
        } else {
            self.selected.clear();
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.primary = None;
        self.selected.clear();
    }

    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.contains(id)
    }

    #[inline]
    pub fn contains(&self, id: EntityId) -> bool {
        self.selected
            .binary_search_by_key(&id.data().as_ffi(), |e| e.data().as_ffi())
            .is_ok()
    }

    /// Replace selection with a single entity.
    #[inline]
    pub fn set_single(&mut self, id: Option<EntityId>) {
        self.primary = id;
        self.selected.clear();
        if let Some(e) = id {
            self.selected.push(e);
        }
    }

    /// Add an entity to selection (keeps deterministic order). Sets it as primary.
    #[inline]
    pub fn add(&mut self, id: EntityId) {
        match self
            .selected
            .binary_search_by_key(&id.data().as_ffi(), |e| e.data().as_ffi())
        {
            Ok(_) => {
                self.primary = Some(id);
            }
            Err(ix) => {
                self.selected.insert(ix, id);
                self.primary = Some(id);
            }
        }
    }

    /// Remove an entity from selection.
    #[inline]
    pub fn remove(&mut self, id: EntityId) {
        if let Ok(ix) = self
            .selected
            .binary_search_by_key(&id.data().as_ffi(), |e| e.data().as_ffi())
        {
            self.selected.remove(ix);
        }

        if self.primary == Some(id) {
            self.primary = self.selected.last().copied();
        }
    }

    /// Toggle an entity selection state. If toggled on - becomes primary.
    #[inline]
    pub fn toggle(&mut self, id: EntityId) {
        if self.contains(id) {
            self.remove(id);
        } else {
            self.add(id);
        }
    }
}
