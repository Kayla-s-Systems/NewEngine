#![forbid(unsafe_op_in_unsafe_fn)]

use slotmap::SecondaryMap;

use crate::EntityId;

/// Immutable query iterator over a single component type.
pub struct Query<'a, T: 'static> {
    pub(crate) iter: Option<slotmap::secondary::Iter<'a, EntityId, T>>,
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
    pub(crate) iter: Option<slotmap::secondary::IterMut<'a, EntityId, T>>,
}

impl<'a, T: 'static> Iterator for QueryMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Join query iterator over two component types.
pub enum Query2<'a, A: 'static, B: 'static> {
    Empty,
    A(Query2A<'a, A, B>),
    B(Query2B<'a, A, B>),
}

pub struct Query2A<'a, A: 'static, B: 'static> {
    pub(crate) iter: slotmap::secondary::Iter<'a, EntityId, A>,
    pub(crate) b: &'a SecondaryMap<EntityId, B>,
}

pub struct Query2B<'a, A: 'static, B: 'static> {
    pub(crate) iter: slotmap::secondary::Iter<'a, EntityId, B>,
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