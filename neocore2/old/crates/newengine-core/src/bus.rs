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

    /// Sends an event to the bus.
    ///
    /// Returns an error if all receivers are disconnected.
    #[inline]
    pub fn send(&self, ev: E) -> Result<(), crossbeam_channel::SendError<E>> {
        self.tx.send(ev)
    }

    /// Lossy send variant for non-critical fire-and-forget signals.
    #[inline]
    pub fn send_lossy(&self, ev: E) {
        let _ = self.tx.send(ev);
    }

    #[inline]
    pub fn try_recv(&self) -> Option<E> {
        self.rx.try_recv().ok()
    }

    #[inline]
    pub fn drain_into(&self, out: &mut Vec<E>) {
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
        }
    }
}
