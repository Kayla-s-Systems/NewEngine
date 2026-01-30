use crate::error::{EngineError, EngineResult};

use crossbeam_channel::{Receiver, Sender};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, Weak,
};

/// Multicast event hub with typed subscriptions and optional filters.
///
/// Publish:
///   hub.publish(MyEvent { .. });
///
/// Subscribe:
///   let sub = hub.subscribe::<MyEvent>();
///   sub.drain(|ev| { ... });
///
/// Filtered subscribe:
///   let sub = hub.subscribe_filtered::<MyEvent>(|ev| ev.kind == 42);
pub struct EventHub {
    inner: Arc<Inner>,
}

impl Default for EventHub {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EventHub {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(1),
                chans: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Publish an event to all subscribers of this type.
    #[inline]
    pub fn publish<T>(&self, event: T) -> EngineResult<()>
    where
        T: Any + Send + Sync + 'static,
    {
        let arc: Arc<dyn Any + Send + Sync> = Arc::new(event);
        self.inner.publish_typed(TypeId::of::<T>(), arc)
    }

    /// Subscribe to a typed event stream.
    #[inline]
    pub fn subscribe<T>(&self) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.subscribe_filtered::<T, _>(|_| true)
    }

    /// Subscribe with a filter predicate.
    ///
    /// Filter is executed on publisher thread.
    #[inline]
    pub fn subscribe_filtered<T, F>(&self, filter: F) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let (tx, rx) = crossbeam_channel::unbounded::<Arc<dyn Any + Send + Sync>>();

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);

        let filter_arc: Arc<dyn Fn(&Arc<dyn Any + Send + Sync>) -> bool + Send + Sync> =
            Arc::new(move |a: &Arc<dyn Any + Send + Sync>| {
                if let Some(ev) = a.as_ref().downcast_ref::<T>() {
                    filter(ev)
                } else {
                    false
                }
            });

        self.inner.add_subscriber(
            TypeId::of::<T>(),
            Subscriber {
                id,
                tx,
                filter: Some(filter_arc),
            },
        );

        EventSub {
            inner: Some(SubInner {
                hub: Arc::downgrade(&self.inner),
                type_id: TypeId::of::<T>(),
                sub_id: id,
            }),
            rx,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Typed subscription handle.
/// On drop, automatically unregisters.
pub struct EventSub<T>
where
    T: Any + Send + Sync + 'static,
{
    inner: Option<SubInner>,
    rx: Receiver<Arc<dyn Any + Send + Sync>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> EventSub<T>
where
    T: Any + Send + Sync + 'static,
{
    #[inline]
    pub fn try_recv(&self) -> Option<Arc<T>> {
        let a = self.rx.try_recv().ok()?;
        Arc::downcast::<T>(a).ok()
    }

    #[inline]
    pub fn drain_into(&self, out: &mut Vec<Arc<T>>) -> usize {
        let mut n = 0usize;
        while let Ok(a) = self.rx.try_recv() {
            if let Ok(ev) = Arc::downcast::<T>(a) {
                out.push(ev);
                n += 1;
            }
        }
        n
    }

    #[inline]
    pub fn drain<F: FnMut(Arc<T>)>(&self, mut f: F) {
        while let Ok(a) = self.rx.try_recv() {
            if let Ok(ev) = Arc::downcast::<T>(a) {
                f(ev);
            }
        }
    }
}

impl<T> Drop for EventSub<T>
where
    T: Any + Send + Sync + 'static,
{
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else { return };
        let Some(hub) = inner.hub.upgrade() else { return };
        hub.remove_subscriber(inner.type_id, inner.sub_id);
    }
}

struct SubInner {
    hub: Weak<Inner>,
    type_id: TypeId,
    sub_id: u64,
}

struct Inner {
    next_id: AtomicU64,
    chans: Mutex<HashMap<TypeId, Vec<Subscriber>>>,
}

impl Inner {
    fn add_subscriber(&self, type_id: TypeId, sub: Subscriber) {
        let mut map = self.chans.lock().unwrap();
        map.entry(type_id).or_default().push(sub);
    }

    fn remove_subscriber(&self, type_id: TypeId, sub_id: u64) {
        let mut map = self.chans.lock().unwrap();
        let Some(list) = map.get_mut(&type_id) else { return };
        list.retain(|s| s.id != sub_id);
        if list.is_empty() {
            map.remove(&type_id);
        }
    }

    fn publish_typed(&self, type_id: TypeId, ev: Arc<dyn Any + Send + Sync>) -> EngineResult<()> {
        let subs_snapshot = {
            let map = self.chans.lock().unwrap();
            map.get(&type_id).cloned()
        };

        let Some(subs) = subs_snapshot else { return Ok(()) };

        let mut failed_ids: Vec<u64> = Vec::new();

        for s in subs.iter() {
            if let Some(filter) = &s.filter {
                if !filter(&ev) {
                    continue;
                }
            }

            if s.tx.send(ev.clone()).is_err() {
                failed_ids.push(s.id);
            }
        }

        if !failed_ids.is_empty() {
            let mut map = self.chans.lock().unwrap();
            if let Some(list) = map.get_mut(&type_id) {
                list.retain(|s| !failed_ids.contains(&s.id));
                if list.is_empty() {
                    map.remove(&type_id);
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
struct Subscriber {
    id: u64,
    tx: Sender<Arc<dyn Any + Send + Sync>>,
    filter: Option<Arc<dyn Fn(&Arc<dyn Any + Send + Sync>) -> bool + Send + Sync>>,
}

trait SenderExt {
    fn is_disconnected(&self) -> bool;
}

impl SenderExt for Sender<Arc<dyn Any + Send + Sync>> {
    #[inline]
    fn is_disconnected(&self) -> bool {
        // crossbeam Sender has no direct "is_closed"; try_send to detect is expensive.
        // We rely on the `send` failure path in publish and then prune.
        false
    }
}