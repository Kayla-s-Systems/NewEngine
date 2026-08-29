use crate::access::{AccessConflictMask, AccessDomain, AccessMask};

use super::{SimAccessConflictDiagnostic, SystemEntry};

pub(super) struct PlannedBatch {
    pub(super) indices: Vec<usize>,
    pub(super) conflict_before: Option<SimAccessConflictDiagnostic>,
}

fn named_domains(mask: u128) -> Vec<String> {
    AccessDomain::all()
        .into_iter()
        .filter(|domain| mask & domain.mask() != 0)
        .map(|domain| domain.as_str().to_owned())
        .collect()
}

fn conflict_diagnostic(
    systems: &[SystemEntry],
    current: &[usize],
    incoming_index: usize,
) -> SimAccessConflictDiagnostic {
    let incoming = systems[incoming_index];
    let mut aggregate = AccessConflictMask::default();
    let mut conflicting_systems = Vec::new();

    for &index in current {
        let existing = systems[index];
        let mask = existing.access.conflict_mask(incoming.access);
        if !mask.is_empty() {
            aggregate = aggregate.union(mask);
            conflicting_systems.push(existing.name.to_owned());
        }
    }

    SimAccessConflictDiagnostic {
        incoming_system: incoming.name.to_owned(),
        conflicting_systems,
        mask: aggregate,
        named_domains: named_domains(aggregate.blocking_mask()),
    }
}

pub(super) fn plan_conflict_free_batches(systems: &[SystemEntry]) -> Vec<PlannedBatch> {
    let mut batches = Vec::<PlannedBatch>::new();
    let mut current = Vec::<usize>::new();
    let mut current_access = AccessMask::none();
    let mut conflict_before = None;

    for (index, system) in systems.iter().enumerate() {
        if !current.is_empty() && current_access.conflicts(system.access) {
            let next_conflict = conflict_diagnostic(systems, &current, index);
            batches.push(PlannedBatch {
                indices: core::mem::take(&mut current),
                conflict_before: conflict_before.take(),
            });
            current_access = AccessMask::none();
            conflict_before = Some(next_conflict);
        }
        current.push(index);
        current_access = current_access.union(system.access);
    }

    if !current.is_empty() {
        batches.push(PlannedBatch {
            indices: current,
            conflict_before,
        });
    }
    batches
}
