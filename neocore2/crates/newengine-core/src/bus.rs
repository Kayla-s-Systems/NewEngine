use crossbeam_channel::{Receiver, Sender};

pub struct Bus<E: Send + 'static> {
    tx: Sender<E>,
    rx: Receiver<E>,
}

impl<E: Send + 'static> Bus<E> {
    #[inline]
    pub fn new(tx: Sender<E>, rx: Receiver<E>) -> Self {
        Self { tx, rx }
    }

    /// Best-effort send.
    ///
    /// Returns `true` if the event was accepted by the channel.
    #[inline]
    pub fn try_send(&self, ev: E) -> bool {
        self.tx.try_send(ev).is_ok()
    }

    /// Fire-and-forget send.
    ///
    /// Intended for non-critical events. For critical signaling, prefer `try_send` and handle `false`.
    #[inline]
    pub fn send(&self, ev: E) {
        let _ = self.tx.send(ev);
    }

    #[inline]
    pub fn try_recv(&self) -> Option<E> {
        self.rx.try_recv().ok()
    }

    #[inline]
    pub fn drain_into(&self, out: &mut Vec<E>) -> usize {
        let mut n = 0usize;
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
            n += 1;
        }
        n
    }
}