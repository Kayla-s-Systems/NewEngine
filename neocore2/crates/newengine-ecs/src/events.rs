#![forbid(unsafe_op_in_unsafe_fn)]

/// Double-buffered, deterministic event queue.
///
/// Usage pattern (per frame or per fixed tick):
/// 1) systems call `send()` during execution
/// 2) runtime calls `swap()` once at the end of the stage
/// 3) systems read via `drain()` in the next stage/tick
///
/// This prevents re-entrancy and makes event visibility explicit.
#[derive(Debug, Default)]
pub struct Events<T> {
    write: Vec<T>,
    read: Vec<T>,
}

impl<T> Events<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            write: Vec::new(),
            read: Vec::new(),
        }
    }

    /// Sends an event to be visible after the next `swap()`.
    #[inline]
    pub fn send(&mut self, ev: T) {
        self.write.push(ev);
    }

    /// Swaps buffers. Call exactly once per stage boundary.
    #[inline]
    pub fn swap(&mut self) {
        self.read.clear();
        core::mem::swap(&mut self.read, &mut self.write);
    }

    /// Drains currently visible events (read buffer).
    #[inline]
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.read.drain(..)
    }

    /// Returns a slice of visible events.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.read
    }

    #[inline]
    pub fn clear_all(&mut self) {
        self.write.clear();
        self.read.clear();
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.write.is_empty() && self.read.is_empty()
    }

    #[inline]
    pub fn len_visible(&self) -> usize {
        self.read.len()
    }

    #[inline]
    pub fn len_pending(&self) -> usize {
        self.write.len()
    }
}
