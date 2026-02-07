#![forbid(unsafe_op_in_unsafe_fn)]

use ahash::AHashMap;
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEventKind {
    Click,
    Change,
    Submit,
}

#[derive(Debug, Clone)]
pub struct UiEvent {
    pub kind: UiEventKind,
    pub target_id: String,
    pub value: Option<String>,
    pub actions: SmallVec<[String; 2]>,
}

#[derive(Debug, Default)]
pub struct UiState {
    pub strings: AHashMap<String, String>,
    pub clicked: AHashMap<String, bool>,
    pub vars: AHashMap<String, String>,
    pub unknown_tags: AHashMap<String, u32>,

    events: Vec<UiEvent>,
}

impl UiState {
    #[inline]
    pub fn take_clicked(&mut self, id: &str) -> bool {
        self.clicked.remove(id).unwrap_or(false)
    }

    #[inline]
    pub fn set_var(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.vars.insert(k.into(), v.into());
    }

    #[inline]
    pub fn drain_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }

    #[inline]
    pub(crate) fn push_event(&mut self, ev: UiEvent) {
        self.events.push(ev);
    }
}