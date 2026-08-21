use super::*;
use newengine_loading_api::EngineTaskPhase;

#[derive(Clone, Copy, Debug)]
pub(super) enum TaskBodyOutcome {
    CompletedNoClosure,
    Completed,
    Failed,
    CancelledBeforeExecution,
    CancelledWhilePaused,
    CancelledAfterExecution,
}

impl TaskBodyOutcome {
    #[inline]
    pub(super) fn phase(self) -> EngineTaskPhase {
        match self {
            Self::CompletedNoClosure | Self::Completed => EngineTaskPhase::Completed,
            Self::Failed => EngineTaskPhase::Failed,
            Self::CancelledBeforeExecution
            | Self::CancelledWhilePaused
            | Self::CancelledAfterExecution => EngineTaskPhase::Cancelled,
        }
    }

    #[inline]
    pub(super) fn status(self) -> &'static str {
        match self {
            Self::CompletedNoClosure | Self::Completed => "Task completed",
            Self::Failed => "Task failed",
            Self::CancelledBeforeExecution
            | Self::CancelledWhilePaused
            | Self::CancelledAfterExecution => "Task cancelled",
        }
    }

    #[inline]
    pub(super) fn detail(self) -> &'static str {
        match self {
            Self::CompletedNoClosure => "Task completed without a task closure.",
            Self::Completed => "Task finished on engine-runtime worker thread.",
            Self::Failed => "Worker task panicked; worker recovered and continues.",
            Self::CancelledBeforeExecution => "Task was cancelled before worker execution.",
            Self::CancelledWhilePaused => "Task was cancelled while paused before execution.",
            Self::CancelledAfterExecution => "Task completed after observing cancellation.",
        }
    }

    #[inline]
    pub(super) fn counts_completed(self) -> bool {
        !matches!(
            self,
            Self::CancelledBeforeExecution | Self::CancelledWhilePaused
        )
    }

    #[inline]
    pub(super) fn counts_cancelled(self) -> bool {
        matches!(
            self,
            Self::CancelledBeforeExecution
                | Self::CancelledWhilePaused
                | Self::CancelledAfterExecution
        )
    }

    #[inline]
    pub(super) fn counts_panicked(self) -> bool {
        matches!(self, Self::Failed)
    }
}

struct TaskHierarchyNode {
    parent_task_id: Option<String>,
    pending_children: usize,
    body_outcome: Option<TaskBodyOutcome>,
    finalized: bool,
}

#[derive(Default)]
pub(super) struct TaskHierarchyGraph {
    nodes: HashMap<String, TaskHierarchyNode>,
    /// Children may be submitted before their parent is registered. Keep the
    /// relationship so parent completion remains order-independent.
    waiting_children: HashMap<String, Vec<String>>,
}

impl TaskHierarchyGraph {
    pub(super) fn register(&mut self, task_id: &str, parent_task_id: Option<&str>) {
        if self.nodes.contains_key(task_id) {
            return;
        }

        let parent_task_id = parent_task_id
            .map(str::trim)
            .filter(|parent| !parent.is_empty() && *parent != task_id)
            .map(str::to_owned);

        let pending_children = self
            .waiting_children
            .remove(task_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|child_id| {
                self.nodes
                    .get(child_id)
                    .is_some_and(|child| !child.finalized)
            })
            .count();

        self.nodes.insert(
            task_id.to_owned(),
            TaskHierarchyNode {
                parent_task_id: parent_task_id.clone(),
                pending_children,
                body_outcome: None,
                finalized: false,
            },
        );

        let Some(parent_task_id) = parent_task_id else {
            return;
        };

        if let Some(parent) = self.nodes.get_mut(&parent_task_id) {
            if !parent.finalized {
                parent.pending_children = parent.pending_children.saturating_add(1);
            }
        } else {
            self.waiting_children
                .entry(parent_task_id)
                .or_default()
                .push(task_id.to_owned());
        }
    }

    pub(super) fn finish_body(
        &mut self,
        task_id: &str,
        outcome: TaskBodyOutcome,
    ) -> (bool, Vec<(String, TaskBodyOutcome)>) {
        let Some(node) = self.nodes.get_mut(task_id) else {
            return (false, Vec::new());
        };
        if node.finalized {
            return (false, Vec::new());
        }
        node.body_outcome = Some(outcome);
        let waiting_for_children = node.pending_children > 0;

        let mut finalized = Vec::new();
        let mut candidate = Some(task_id.to_owned());
        while let Some(candidate_id) = candidate.take() {
            let (parent_task_id, candidate_outcome) = {
                let Some(candidate_node) = self.nodes.get_mut(&candidate_id) else {
                    break;
                };
                if candidate_node.finalized || candidate_node.pending_children != 0 {
                    break;
                }
                let Some(candidate_outcome) = candidate_node.body_outcome else {
                    break;
                };
                candidate_node.finalized = true;
                (candidate_node.parent_task_id.clone(), candidate_outcome)
            };

            finalized.push((candidate_id, candidate_outcome));

            let Some(parent_task_id) = parent_task_id else {
                continue;
            };
            let Some(parent) = self.nodes.get_mut(&parent_task_id) else {
                continue;
            };
            if parent.finalized {
                continue;
            }

            parent.pending_children = parent.pending_children.saturating_sub(1);
            if parent.pending_children == 0 && parent.body_outcome.is_some() {
                candidate = Some(parent_task_id);
            }
        }

        (waiting_for_children, finalized)
    }
}
