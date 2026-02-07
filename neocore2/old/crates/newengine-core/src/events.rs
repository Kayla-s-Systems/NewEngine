use crate::error::EngineResult;

use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock, Weak,
};

/// Overflow policy for bounded subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// If the queue is full, silently drop the newly published event.
    DropNewest,
    /// Block the publisher until the subscriber makes room.
    ///
    /// Use with care: it can deadlock if you publish from the same thread
    /// that is expected to drain the subscription.
    Block,
}

impl Default for OverflowPolicy {
    #[inline]
    fn default() -> Self {
        OverflowPolicy::DropNewest
    }
}

/// Multicast event hub with typed subscriptions and optional filters.
///
/// Backpressure is supported via bounded subscriptions with explicit overflow policies.
///
/// Optimized for cheap publish:
/// - subscriber lists are stored as `Arc<Vec<Subscriber>>`
/// - subscribe/unsubscribe uses copy-on-write
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
                chans: RwLock::new(HashMap::new()),
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
    ///
    /// The default subscription is **bounded** with `OverflowPolicy::DropNewest`.
    /// This prevents unbounded memory growth if a subscriber stalls.
    #[inline]
    pub fn subscribe<T>(&self) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.subscribe_bounded::<T>(1024, OverflowPolicy::DropNewest)
    }

    /// Subscribe to a typed event stream with a bounded queue.
    #[inline]
    pub fn subscribe_bounded<T>(&self, capacity: usize, overflow: OverflowPolicy) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.subscribe_filtered_bounded::<T, _>(capacity, overflow, |_| true)
    }

    /// Subscribe with a filter predicate.
    ///
    /// Filter is executed on the publisher thread.
    #[inline]
    pub fn subscribe_filtered<T, F>(&self, filter: F) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.subscribe_filtered_bounded::<T, F>(1024, OverflowPolicy::DropNewest, filter)
    }

    /// Subscribe with a filter predicate and a bounded queue.
    #[inline]
    pub fn subscribe_filtered_bounded<T, F>(
        &self,
        capacity: usize,
        overflow: OverflowPolicy,
        filter: F,
    ) -> EventSub<T>
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let cap = capacity.max(1);
        let (tx, rx) = crossbeam_channel::bounded::<Arc<dyn Any + Send + Sync>>(cap);

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let dropped = Arc::new(AtomicU64::new(0));

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
                overflow,
                dropped: dropped.clone(),
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
            dropped,
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
    dropped: Arc<AtomicU64>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> EventSub<T>
where
    T: Any + Send + Sync + 'static,
{
    /// Number of events dropped due to overflow on this subscription.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

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
        let Some(inner) = self.inner.take() else {
            return;
        };
        let Some(hub) = inner.hub.upgrade() else {
            return;
        };
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
    chans: RwLock<HashMap<TypeId, Arc<Vec<Subscriber>>>>,
}

impl Inner {
    fn add_subscriber(&self, type_id: TypeId, sub: Subscriber) {
        let mut map = self.chans.write().expect("EventHub channels poisoned");
        let next = match map.get(&type_id) {
            Some(cur) => {
                let mut v: Vec<Subscriber> = (**cur).clone();
                v.push(sub);
                Arc::new(v)
            }
            None => Arc::new(vec![sub]),
        };
        map.insert(type_id, next);
    }

    fn remove_subscriber(&self, type_id: TypeId, sub_id: u64) {
        let mut map = self.chans.write().expect("EventHub channels poisoned");
        let Some(cur) = map.get(&type_id) else { return };

        let mut v: Vec<Subscriber> = Vec::with_capacity(cur.len().saturating_sub(1));
        for s in cur.iter() {
            if s.id != sub_id {
                v.push(s.clone());
            }
        }

        if v.is_empty() {
            map.remove(&type_id);
        } else {
            map.insert(type_id, Arc::new(v));
        }
    }

    fn publish_typed(&self, type_id: TypeId, ev: Arc<dyn Any + Send + Sync>) -> EngineResult<()> {
        let subs = {
            let map = self.chans.read().expect("EventHub channels poisoned");
            map.get(&type_id).cloned()
        };

        let Some(subs) = subs else { return Ok(()) };

        let mut failed: HashSet<u64> = HashSet::new();

        for s in subs.iter() {
            if let Some(filter) = &s.filter {
                if !filter(&ev) {
                    continue;
                }
            }

            match s.overflow {
                OverflowPolicy::Block => {
                    if s.tx.send(ev.clone()).is_err() {
                        failed.insert(s.id);
                    }
                }
                OverflowPolicy::DropNewest => match s.tx.try_send(ev.clone()) {
                    Ok(_) => {}
                    Err(TrySendError::Full(_)) => {
                        s.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        failed.insert(s.id);
                    }
                },
            }
        }

        if failed.is_empty() {
            return Ok(());
        }

        let mut map = self.chans.write().expect("EventHub channels poisoned");
        let Some(cur) = map.get(&type_id) else {
            return Ok(());
        };

        let mut v: Vec<Subscriber> = Vec::with_capacity(cur.len());
        for s in cur.iter() {
            if !failed.contains(&s.id) {
                v.push(s.clone());
            }
        }

        if v.is_empty() {
            map.remove(&type_id);
        } else {
            map.insert(type_id, Arc::new(v));
        }

        Ok(())
    }
}

#[derive(Clone)]
struct Subscriber {
    id: u64,
    tx: Sender<Arc<dyn Any + Send + Sync>>,
    overflow: OverflowPolicy,
    dropped: Arc<AtomicU64>,
    filter: Option<Arc<dyn Fn(&Arc<dyn Any + Send + Sync>) -> bool + Send + Sync>>,
}
