use super::super::module_slot::{ModuleSlot, ModuleState};

use crate::error::{EngineError, EngineResult};
use crate::module::ApiVersion;

use newengine_math::collections_prelude::{
    ne_hash_map_with_capacity, NeHashMap as HashMap, NeVecDeque as VecDeque,
};

#[derive(Debug)]
pub(super) struct ResilientModulePlan {
    pub(super) order: Vec<usize>,
    pub(super) enabled: Vec<bool>,
    pub(super) reasons: Vec<Option<String>>,
}

pub(super) fn build_strict_module_order<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
) -> EngineResult<Vec<usize>> {
    let n = modules.len();
    let id_to_index = index_modules_strict(modules)?;
    let mut graph = DependencyGraph::new(n);

    for (i, slot) in modules.iter().enumerate() {
        let module = slot.module.as_ref();
        for &dep in module.dependencies() {
            let Some(&dep_i) = id_to_index.get(dep) else {
                return Err(EngineError::Other(format!(
                    "module dependency missing: {} -> {dep}",
                    module.id()
                )));
            };
            graph.add_edge(dep_i, i);
        }
    }

    let order = graph.topological_order(|_| true);
    if order.len() != n {
        let cyclic = graph
            .indegree
            .iter()
            .enumerate()
            .filter_map(|(i, deg)| (*deg != 0).then(|| modules[i].id()))
            .collect::<Vec<_>>();
        return Err(EngineError::Other(format!(
            "module dependency cycle detected among: {:?}",
            cyclic
        )));
    }

    Ok(order)
}

pub(super) fn reorder_slots_by_order<E: Send + 'static>(
    modules: Vec<ModuleSlot<E>>,
    order: &[usize],
) -> Vec<ModuleSlot<E>> {
    let mut slots = modules.into_iter().map(Some).collect::<Vec<_>>();
    let mut sorted = Vec::with_capacity(slots.len());

    for &idx in order {
        let slot = slots[idx].take().expect("module slot already moved");
        sorted.push(slot);
    }

    sorted
}

pub(super) fn build_resilient_module_plan<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
) -> ResilientModulePlan {
    let n = modules.len();
    let id_to_index = index_modules_resilient(modules);
    let mut enabled = vec![true; n];
    let mut reasons = vec![None; n];

    prune_invalid_modules(modules, &id_to_index, &mut enabled, &mut reasons);

    let mut graph = DependencyGraph::new(n);
    for (i, slot) in modules.iter().enumerate() {
        if !enabled[i] {
            continue;
        }

        for &dep in slot.module.dependencies() {
            let Some(&dep_i) = id_to_index.get(dep) else {
                continue;
            };
            if enabled[dep_i] {
                graph.add_edge(dep_i, i);
            }
        }
    }

    let order = graph.topological_order(|i| enabled[i]);
    for i in 0..n {
        if enabled[i] && graph.indegree[i] != 0 {
            enabled[i] = false;
            reasons[i] = Some("dependency cycle detected".to_string());
        }
    }

    ResilientModulePlan {
        order,
        enabled,
        reasons,
    }
}

pub(super) fn partition_slots_by_resilient_plan<E: Send + 'static>(
    modules: Vec<ModuleSlot<E>>,
    mut plan: ResilientModulePlan,
) -> Vec<ModuleSlot<E>> {
    let mut opt = modules.into_iter().map(Some).collect::<Vec<_>>();
    let mut new_slots = Vec::with_capacity(opt.len());
    let mut moved = vec![false; opt.len()];

    for &idx in &plan.order {
        if idx >= opt.len() || !plan.enabled[idx] {
            continue;
        }
        moved[idx] = true;
        let mut slot = opt[idx].take().expect("module slot already moved");
        slot.state = ModuleState::Pending;
        new_slots.push(slot);
    }

    for i in 0..opt.len() {
        if moved[i] {
            continue;
        }
        let mut slot = opt[i].take().expect("module slot already moved");
        let reason = plan.reasons[i]
            .take()
            .unwrap_or_else(|| "disabled".to_string());
        slot.disable(reason.clone());
        newengine_ulog_api::ulog::warn!("engine: module disabled: {} ({})", slot.id(), reason);
        new_slots.push(slot);
    }

    new_slots
}

fn index_modules_strict<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
) -> EngineResult<HashMap<&'static str, usize>> {
    let mut id_to_index = ne_hash_map_with_capacity::<&'static str, usize>(modules.len());
    for (i, slot) in modules.iter().enumerate() {
        let id = slot.id();
        if id_to_index.insert(id, i).is_some() {
            return Err(EngineError::Other(format!("duplicate module id: {id}")));
        }
    }
    Ok(id_to_index)
}

fn index_modules_resilient<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
) -> HashMap<&'static str, usize> {
    let mut id_to_index = ne_hash_map_with_capacity::<&'static str, usize>(modules.len());
    for (i, slot) in modules.iter().enumerate() {
        let id = slot.id();
        if id_to_index.insert(id, i).is_some() {
            newengine_ulog_api::ulog::error!("engine: duplicate module id: {}", id);
        }
    }
    id_to_index
}

fn prune_invalid_modules<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
    id_to_index: &HashMap<&'static str, usize>,
    enabled: &mut [bool],
    reasons: &mut [Option<String>],
) {
    loop {
        let mut changed = false;

        changed |= prune_missing_or_disabled_dependencies(modules, id_to_index, enabled, reasons);
        changed |= prune_missing_or_incompatible_apis(modules, enabled, reasons);

        if !changed {
            break;
        }
    }
}

fn prune_missing_or_disabled_dependencies<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
    id_to_index: &HashMap<&'static str, usize>,
    enabled: &mut [bool],
    reasons: &mut [Option<String>],
) -> bool {
    let mut changed = false;

    for (i, slot) in modules.iter().enumerate() {
        if !enabled[i] {
            continue;
        }

        let module = slot.module.as_ref();
        for &dep in module.dependencies() {
            let Some(&dep_i) = id_to_index.get(dep) else {
                enabled[i] = false;
                reasons[i] = Some(format!("missing dependency: {} -> {}", module.id(), dep));
                changed = true;
                break;
            };

            if !enabled[dep_i] {
                enabled[i] = false;
                reasons[i] = Some(format!("dependency disabled: {} -> {}", module.id(), dep));
                changed = true;
                break;
            }
        }
    }

    changed
}

fn prune_missing_or_incompatible_apis<E: Send + 'static>(
    modules: &[ModuleSlot<E>],
    enabled: &mut [bool],
    reasons: &mut [Option<String>],
) -> bool {
    let mut changed = false;
    let mut provided: HashMap<&'static str, ApiVersion> = HashMap::default();
    let mut provider: HashMap<&'static str, &'static str> = HashMap::default();

    for (i, slot) in modules.iter().enumerate() {
        if !enabled[i] {
            continue;
        }
        let module = slot.module.as_ref();
        for provide in module.provides() {
            match provided.get(provide.id) {
                Some(version) if *version >= provide.version => {}
                _ => {
                    provided.insert(provide.id, provide.version);
                    provider.insert(provide.id, module.id());
                }
            }
        }
    }

    for (i, slot) in modules.iter().enumerate() {
        if !enabled[i] {
            continue;
        }
        let module = slot.module.as_ref();
        for require in module.requires() {
            let Some(have) = provided.get(require.id) else {
                enabled[i] = false;
                reasons[i] = Some(format!(
                    "requires API '{}' >= {}.{}.{} but it is not provided",
                    require.id,
                    require.min_version.major,
                    require.min_version.minor,
                    require.min_version.patch
                ));
                changed = true;
                break;
            };

            if *have < require.min_version {
                let provider_id = provider.get(require.id).copied().unwrap_or("<unknown>");
                enabled[i] = false;
                reasons[i] = Some(format!(
                    "requires API '{}' >= {}.{}.{} but provider '{}' offers {}.{}.{}",
                    require.id,
                    require.min_version.major,
                    require.min_version.minor,
                    require.min_version.patch,
                    provider_id,
                    have.major,
                    have.minor,
                    have.patch,
                ));
                changed = true;
                break;
            }
        }
    }

    changed
}

struct DependencyGraph {
    indegree: Vec<usize>,
    reverse_edges: Vec<Vec<usize>>,
}

impl DependencyGraph {
    fn new(node_count: usize) -> Self {
        Self {
            indegree: vec![0; node_count],
            reverse_edges: vec![Vec::new(); node_count],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        self.indegree[to] += 1;
        self.reverse_edges[from].push(to);
    }

    fn topological_order(&mut self, include: impl Fn(usize) -> bool) -> Vec<usize> {
        let mut queue = VecDeque::new();
        for i in 0..self.indegree.len() {
            if include(i) && self.indegree[i] == 0 {
                queue.push_back(i);
            }
        }

        let mut order = Vec::with_capacity(self.indegree.len());
        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &to in &self.reverse_edges[i] {
                self.indegree[to] = self.indegree[to].saturating_sub(1);
                if include(to) && self.indegree[to] == 0 {
                    queue.push_back(to);
                }
            }
        }

        order
    }
}
