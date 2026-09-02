use super::BlockVoiceNode;
use std::cmp::Ordering as CmpOrdering;
use std::time::Duration;

pub(super) struct RenderCommand {
    pub(super) at_sample: Option<u64>,
    pub(super) sequence: u64,
    pub(super) schedule_id: Option<u64>,
    pub(super) kind: RenderCommandKind,
}

impl RenderCommand {
    pub(super) fn resolved_sample(&self, current_sample: u64) -> u64 {
        self.at_sample.unwrap_or(current_sample).max(current_sample)
    }
}

impl PartialEq for RenderCommand {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence && self.at_sample == other.at_sample
    }
}

impl Eq for RenderCommand {}

impl PartialOrd for RenderCommand {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for RenderCommand {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.at_sample
            .unwrap_or(0)
            .cmp(&other.at_sample.unwrap_or(0))
            .then_with(|| self.sequence.cmp(&other.sequence))
    }
}

pub(super) enum RenderCommandKind {
    Add {
        node: BlockVoiceNode,
    },
    Remove {
        node_id: u64,
    },
    SetGain {
        node_id: u64,
        gain: f32,
    },
    RampGain {
        node_id: u64,
        target: f32,
        duration_samples: u64,
    },
    SetSpeed {
        node_id: u64,
        speed: f32,
    },
    SetPaused {
        node_id: u64,
        paused: bool,
    },
    Seek {
        node_id: u64,
        position: Duration,
    },
    CancelScheduled {
        node_id: u64,
        schedule_id: u64,
    },
}

pub(super) fn render_command_node_id(kind: &RenderCommandKind) -> Option<u64> {
    match kind {
        RenderCommandKind::Add { node } => Some(node.id),
        RenderCommandKind::Remove { node_id }
        | RenderCommandKind::SetGain { node_id, .. }
        | RenderCommandKind::RampGain { node_id, .. }
        | RenderCommandKind::SetSpeed { node_id, .. }
        | RenderCommandKind::SetPaused { node_id, .. }
        | RenderCommandKind::Seek { node_id, .. }
        | RenderCommandKind::CancelScheduled { node_id, .. } => Some(*node_id),
    }
}
