#![forbid(unsafe_op_in_unsafe_fn)]

use std::any::Any;

/// Logical UI layer.
///
/// This enum describes where a contribution should appear conceptually.
/// It does not assume any concrete UI backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UiLayer {
    /// Main UI: docks, panels, menus.
    Main = 0,
    /// Overlay UI in screen space (gizmos, selection outlines, debug overlays).
    Overlay = 1,
    /// Always-on-top debugging UI (metrics, inspector overlays).
    Debug = 2,
}

/// Stable ordering inside a layer.
pub type UiOrder = i32;

/// A dynamic frame context passed to contributors.
///
/// `ctx_any` is the provider-typed context (e.g. a provider-native context).
/// `user_data` is host-provided per-frame data/services.
pub struct UiDynFrame<'a> {
    pub ctx_any: &'a mut dyn Any,
    pub user_data: &'a mut dyn Any,
}

impl<'a> UiDynFrame<'a> {
    #[inline]
    pub fn downcast_ctx_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.ctx_any.downcast_mut::<T>()
    }

    #[inline]
    pub fn downcast_user_data_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.user_data.downcast_mut::<T>()
    }
}

/// Object-safe UI contribution.
///
/// Any subsystem can register a contributor into a `UiHub` to render UI for a frame.
pub trait UiContributor: Send {
    /// Stable identifier for diagnostics and debugging.
    fn id(&self) -> &'static str;

    /// Where this contribution should be executed.
    fn layer(&self) -> UiLayer {
        UiLayer::Main
    }

    /// Relative ordering inside the layer.
    fn order(&self) -> UiOrder {
        0
    }

    /// Draw this contribution for the current frame.
    fn draw(&mut self, frame: &mut UiDynFrame<'_>);
}

struct Entry {
    layer: UiLayer,
    order: UiOrder,
    seq: u64,
    contrib: Box<dyn UiContributor>,
}

/// UI hub that aggregates and orders contributions deterministically.
#[derive(Default)]
pub struct UiHub {
    next_seq: u64,
    entries: Vec<Entry>,
    dirty_sort: bool,
}

impl UiHub {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a contributor and return its handle.
    pub fn register(&mut self, contrib: Box<dyn UiContributor>) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);

        let layer = contrib.layer();
        let order = contrib.order();

        self.entries.push(Entry {
            layer,
            order,
            seq,
            contrib,
        });
        self.dirty_sort = true;
        seq
    }

    /// Remove by handle returned from `register()`.
    pub fn remove(&mut self, handle: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.seq != handle);
        before != self.entries.len()
    }

    /// Execute all contributors for this frame.
    pub fn run(&mut self, ctx_any: &mut dyn Any, user_data: &mut dyn Any) {
        if self.dirty_sort {
            self.entries
                .sort_by(|a, b| (a.layer, a.order, a.seq).cmp(&(b.layer, b.order, b.seq)));
            self.dirty_sort = false;
        }

        let mut frame = UiDynFrame { ctx_any, user_data };

        for e in &mut self.entries {
            e.contrib.draw(&mut frame);
        }
    }

    /// Enumerate contributions for diagnostics.
    pub fn list(&self) -> Vec<(&'static str, UiLayer, UiOrder, u64)> {
        self.entries
            .iter()
            .map(|e| (e.contrib.id(), e.layer, e.order, e.seq))
            .collect()
    }
}
