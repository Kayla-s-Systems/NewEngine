#![forbid(unsafe_op_in_unsafe_fn)]

/// Per-frame byte arena for temporary CPU data.
///
/// The arena owns its memory and is reset explicitly at frame boundaries. It is
/// intentionally safe and boring: hot systems can depend on the contract before
/// we introduce typed bump allocation or backend-specific virtual memory pages.
#[derive(Clone, Debug)]
pub struct FrameArena {
    bytes: Vec<u8>,
    cursor: usize,
    high_water_mark: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameArenaStats {
    pub capacity: usize,
    pub used: usize,
    pub high_water_mark: usize,
}

impl FrameArena {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: vec![0; capacity],
            cursor: 0,
            high_water_mark: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn used(&self) -> usize {
        self.cursor
    }

    pub fn alloc_bytes_zeroed(&mut self, len: usize, align: usize) -> Option<&mut [u8]> {
        let align = align.max(1).next_power_of_two();
        let start = align_up(self.cursor, align);
        let end = start.checked_add(len)?;
        if end > self.bytes.len() {
            return None;
        }
        self.cursor = end;
        self.high_water_mark = self.high_water_mark.max(self.cursor);
        let slice = &mut self.bytes[start..end];
        slice.fill(0);
        Some(slice)
    }

    #[inline]
    pub fn stats(&self) -> FrameArenaStats {
        FrameArenaStats {
            capacity: self.bytes.len(),
            used: self.cursor,
            high_water_mark: self.high_water_mark,
        }
    }
}

impl Default for FrameArena {
    #[inline]
    fn default() -> Self {
        Self::with_capacity(4 * 1024 * 1024)
    }
}

/// Stable handle into a pool allocator. The generation prevents stale frees from
/// reusing a slot after it was returned and allocated again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PoolHandle {
    index: usize,
    generation: u32,
}

impl PoolHandle {
    #[inline]
    pub const fn index(self) -> usize {
        self.index
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Debug)]
struct PoolSlot<T> {
    value: Option<T>,
    generation: u32,
}

/// O(1) free-list pool allocator for runtime objects that churn frequently.
#[derive(Clone, Debug)]
pub struct PoolAllocator<T> {
    slots: Vec<PoolSlot<T>>,
    free: Vec<usize>,
    live: usize,
}

impl<T> PoolAllocator<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        let mut free = Vec::with_capacity(capacity);
        for index in 0..capacity {
            slots.push(PoolSlot {
                value: None,
                generation: 1,
            });
            free.push(capacity - 1 - index);
        }
        Self { slots, free, live: 0 }
    }

    pub fn alloc(&mut self, value: T) -> Option<PoolHandle> {
        let index = self.free.pop()?;
        let slot = &mut self.slots[index];
        debug_assert!(slot.value.is_none());
        slot.value = Some(value);
        self.live = self.live.saturating_add(1);
        Some(PoolHandle {
            index,
            generation: slot.generation,
        })
    }

    pub fn free(&mut self, handle: PoolHandle) -> Option<T> {
        let slot = self.slots.get_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        self.free.push(handle.index);
        self.live = self.live.saturating_sub(1);
        Some(value)
    }

    pub fn get(&self, handle: PoolHandle) -> Option<&T> {
        let slot = self.slots.get(handle.index)?;
        (slot.generation == handle.generation)
            .then_some(())
            .and_then(|_| slot.value.as_ref())
    }

    pub fn get_mut(&mut self, handle: PoolHandle) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index)?;
        (slot.generation == handle.generation)
            .then_some(())
            .and_then(|_| slot.value.as_mut())
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    #[inline]
    pub fn live_len(&self) -> usize {
        self.live
    }

    #[inline]
    pub fn free_len(&self) -> usize {
        self.free.len()
    }
}

impl<T> Default for PoolAllocator<T> {
    #[inline]
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

#[inline]
fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}
