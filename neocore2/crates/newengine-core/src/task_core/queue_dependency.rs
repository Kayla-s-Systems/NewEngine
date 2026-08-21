use super::*;

struct BlockedTask {
    task: QueuedTask,
    unresolved_dependencies: usize,
}

#[derive(Default)]
pub(super) struct TaskDependencyGraph {
    /// Tasks that have unresolved prerequisites and therefore must not occupy a
    /// lane/priority ready queue.
    blocked: HashMap<String, BlockedTask>,
    /// Reverse edges: prerequisite task id -> tasks waiting on that prerequisite.
    dependents: HashMap<String, Vec<String>>,
}

impl TaskCoreShared {
    /// Registers reverse prerequisite edges under one graph lock. A task either
    /// becomes immediately runnable or lives exclusively in `blocked` until the
    /// last prerequisite completes.
    pub(super) fn register_dependencies(&self, task: QueuedTask) -> Option<QueuedTask> {
        if task.request.prerequisite_task_ids.is_empty() {
            return Some(task);
        }

        let task_id = task.control.task_id().to_owned();
        let mut graph = self.dependency_graph.lock();
        // Completion state is sampled while holding the dependency graph lock.
        // A concurrent terminal task publishes its atomic completion before it
        // can acquire this same graph lock to release dependents, closing the
        // submit-vs-complete race without a second completed-id registry.
        let completions = self.completions.lock();
        let mut unresolved_dependencies = 0usize;
        let mut registered = Vec::<String>::new();

        for prerequisite in &task.request.prerequisite_task_ids {
            let prerequisite = prerequisite.trim();
            if prerequisite.is_empty()
                || registered
                    .iter()
                    .any(|registered_id| registered_id == prerequisite)
            {
                continue;
            }
            registered.push(prerequisite.to_owned());

            if completions
                .get(prerequisite)
                .is_some_and(|completion| completion.is_complete())
            {
                continue;
            }

            unresolved_dependencies = unresolved_dependencies.saturating_add(1);
            graph
                .dependents
                .entry(prerequisite.to_owned())
                .or_default()
                .push(task_id.clone());
        }

        if unresolved_dependencies == 0 {
            return Some(task);
        }

        graph.blocked.insert(
            task_id,
            BlockedTask {
                task,
                unresolved_dependencies,
            },
        );
        None
    }

    /// Marks a task terminal and directly wakes graph nodes whose final
    /// prerequisite was satisfied. Workers never rescan blocked dependencies.
    pub(super) fn release_dependents(&self, completed_task_id: &str) {
        let ready = {
            let mut graph = self.dependency_graph.lock();
            let dependent_ids = graph
                .dependents
                .remove(completed_task_id)
                .unwrap_or_default();
            let mut ready_ids = Vec::new();

            for dependent_id in dependent_ids {
                let Some(blocked) = graph.blocked.get_mut(&dependent_id) else {
                    continue;
                };
                blocked.unresolved_dependencies = blocked.unresolved_dependencies.saturating_sub(1);
                if blocked.unresolved_dependencies == 0 {
                    ready_ids.push(dependent_id);
                }
            }

            let mut ready = Vec::with_capacity(ready_ids.len());
            for ready_id in ready_ids {
                if let Some(blocked) = graph.blocked.remove(&ready_id) {
                    ready.push(blocked.task);
                }
            }
            ready
        };

        if ready.is_empty() {
            return;
        }
        for task in ready {
            self.enqueue_ready(task);
        }
        self.sleep_wake.notify_all();
    }
}
