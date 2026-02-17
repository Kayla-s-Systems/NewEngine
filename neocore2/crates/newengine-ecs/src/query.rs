#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::slot::SecondaryMap;

use crate::EntityId;

/// Immutable query iterator over a single component type.
pub struct Query<'a, T: 'static> {
    pub(crate) iter: Option<newengine_math::collections::slotmap::secondary::Iter<'a, EntityId, T>>,
}

impl<'a, T: 'static> Iterator for Query<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Mutable query iterator over a single component type.
pub struct QueryMut<'a, T: 'static> {
    pub(crate) iter: Option<newengine_math::collections::slotmap::secondary::IterMut<'a, EntityId, T>>,
}

impl<'a, T: 'static> Iterator for QueryMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Mutable query iterator that conservatively marks every yielded entity as `changed`.
///
/// This is the recommended iterator for simulation systems that mutate components.
///
/// Change tracking semantics:
/// - every `next()` that returns an item will write `changed_tick[id] = tick`
/// - if the system *doesn't* mutate a yielded component, it may cause a false-positive change
///   (conservative by design)
pub struct QueryMutTracked<'a, T: 'static> {
    pub(crate) iter: newengine_math::collections::slotmap::secondary::IterMut<'a, EntityId, T>,
    pub(crate) changed_tick: &'a mut SecondaryMap<EntityId, u64>,
    pub(crate) tick: u64,
}

impl<'a, T: 'static> Iterator for QueryMutTracked<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (id, v) = self.iter.next()?;
        self.changed_tick.insert(id, self.tick);
        Some((id, v))
    }
}

/// Join query iterator over two component types.
pub enum Query2<'a, A: 'static, B: 'static> {
    Empty,
    A(Query2A<'a, A, B>),
    B(Query2B<'a, A, B>),
}

pub struct Query2A<'a, A: 'static, B: 'static> {
    pub(crate) iter: newengine_math::collections::slotmap::secondary::Iter<'a, EntityId, A>,
    pub(crate) b: &'a SecondaryMap<EntityId, B>,
}

pub struct Query2B<'a, A: 'static, B: 'static> {
    pub(crate) iter: newengine_math::collections::slotmap::secondary::Iter<'a, EntityId, B>,
    pub(crate) a: &'a SecondaryMap<EntityId, A>,
}

impl<'a, A: 'static, B: 'static> Iterator for Query2<'a, A, B> {
    type Item = (EntityId, &'a A, &'a B);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Query2::Empty => None,
            Query2::A(q) => {
                while let Some((id, a)) = q.iter.next() {
                    if let Some(b) = q.b.get(id) {
                        return Some((id, a, b));
                    }
                }
                None
            }
            Query2::B(q) => {
                while let Some((id, b)) = q.iter.next() {
                    if let Some(a) = q.a.get(id) {
                        return Some((id, a, b));
                    }
                }
                None
            }
        }
    }
}