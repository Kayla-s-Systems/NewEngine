use crate::{SimStage, SystemEntry, SystemFn};

pub struct Schedule {
    pub(crate) stages: [Vec<SystemEntry>; SimStage::COUNT],
    pub(crate) is_sorted: [bool; SimStage::COUNT],
    pub(crate) next_seq: u64,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            stages: std::array::from_fn(|_| Vec::new()),
            is_sorted: [true; SimStage::COUNT],
            next_seq: 1,
        }
    }

    #[inline]
    pub fn add_system(
        &mut self,
        stage: SimStage,
        order: i32,
        name: &'static str,
        f: SystemFn,
    ) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);

        self.stages[stage.as_usize()].push(SystemEntry {
            order,
            seq,
            name,
            f,
        });

        self.is_sorted[stage.as_usize()] = false;
    }
}