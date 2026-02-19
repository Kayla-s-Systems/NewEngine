#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};

use super::pass::{PassId, PassNode};
use super::resource::ResourceId;

#[derive(Debug, thiserror::Error)]
pub enum GraphCompileError {
    #[error("render graph has a dependency cycle")]
    Cycle,
}

/// Result of graph compilation: a deterministic execution order.
#[derive(Debug, Clone)]
pub struct CompiledGraph {
    pub order: Vec<PassId>,
    pub passes: HashMap<PassId, PassNode>,
    pub producers: HashMap<ResourceId, PassId>,
}

impl CompiledGraph {
    #[inline]
    pub fn pass(&self, id: PassId) -> Option<&PassNode> {
        self.passes.get(&id)
    }
}

/// Kahn topological sort with deterministic tie-breaking by PassId.
pub fn compile(passes: Vec<PassNode>) -> Result<CompiledGraph, GraphCompileError> {
    let mut map: HashMap<PassId, PassNode> = HashMap::with_capacity(passes.len());
    for p in passes {
        map.insert(p.id, p);
    }

    // Producer map (last writer wins during authoring, compile asserts uniqueness if needed later).
    let mut producers: HashMap<ResourceId, PassId> = HashMap::new();
    for (pid, p) in &map {
        for w in &p.writes {
            producers.insert(w.res, *pid);
        }
    }

    // Build adjacency and indegree.
    let mut indeg: HashMap<PassId, u32> = map.keys().map(|&id| (id, 0)).collect();
    let mut edges: HashMap<PassId, Vec<PassId>> = map.keys().map(|&id| (id, Vec::new())).collect();

    for (pid, p) in &map {
        // A pass depends on producers of its reads.
        for r in &p.reads {
            if let Some(prod) = producers.get(&r.res).copied() {
                if prod != *pid {
                    edges.get_mut(&prod).unwrap().push(*pid);
                    *indeg.get_mut(pid).unwrap() += 1;
                }
            }
        }
    }

    // Deterministic queue: collect, sort, then pop_front.
    let mut ready: Vec<PassId> = indeg.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
    ready.sort_by_key(|id| id.0);

    let mut q: VecDeque<PassId> = ready.into();
    let mut order = Vec::with_capacity(map.len());

    while let Some(p) = q.pop_front() {
        order.push(p);

        let mut next = edges.get(&p).cloned().unwrap_or_default();
        next.sort_by_key(|id| id.0);

        for n in next {
            let d = indeg.get_mut(&n).unwrap();
            *d -= 1;
            if *d == 0 {
                // Insert while preserving sorted order (small N: acceptable).
                let pos = q.iter().position(|x| x.0 > n.0).unwrap_or(q.len());
                q.insert(pos, n);
            }
        }
    }

    if order.len() != map.len() {
        return Err(GraphCompileError::Cycle);
    }

    Ok(CompiledGraph { order, passes: map, producers })
}
