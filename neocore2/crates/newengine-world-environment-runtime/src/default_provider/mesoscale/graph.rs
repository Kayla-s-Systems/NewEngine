#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum MesoscaleNodeId {
    SurfaceBoundary,
    PriorCellState,
    Topology,
    Momentum,
    FiniteVolumeTransport,
    ColumnPhysics,
    Observation,
}

pub(super) const NODES: [MesoscaleNodeId; 7] = [
    MesoscaleNodeId::SurfaceBoundary,
    MesoscaleNodeId::PriorCellState,
    MesoscaleNodeId::Topology,
    MesoscaleNodeId::Momentum,
    MesoscaleNodeId::FiniteVolumeTransport,
    MesoscaleNodeId::ColumnPhysics,
    MesoscaleNodeId::Observation,
];

pub(super) const EDGES: [(MesoscaleNodeId, MesoscaleNodeId); 10] = [
    (MesoscaleNodeId::SurfaceBoundary, MesoscaleNodeId::Topology),
    (MesoscaleNodeId::PriorCellState, MesoscaleNodeId::Momentum),
    (MesoscaleNodeId::Topology, MesoscaleNodeId::Momentum),
    (
        MesoscaleNodeId::PriorCellState,
        MesoscaleNodeId::FiniteVolumeTransport,
    ),
    (
        MesoscaleNodeId::Topology,
        MesoscaleNodeId::FiniteVolumeTransport,
    ),
    (
        MesoscaleNodeId::Momentum,
        MesoscaleNodeId::FiniteVolumeTransport,
    ),
    (
        MesoscaleNodeId::SurfaceBoundary,
        MesoscaleNodeId::ColumnPhysics,
    ),
    (
        MesoscaleNodeId::FiniteVolumeTransport,
        MesoscaleNodeId::ColumnPhysics,
    ),
    (MesoscaleNodeId::Momentum, MesoscaleNodeId::ColumnPhysics),
    (MesoscaleNodeId::ColumnPhysics, MesoscaleNodeId::Observation),
];

impl MesoscaleNodeId {
    fn name(self) -> &'static str {
        match self {
            Self::SurfaceBoundary => "surface_boundary",
            Self::PriorCellState => "prior_cell_state",
            Self::Topology => "topology",
            Self::Momentum => "momentum",
            Self::FiniteVolumeTransport => "finite_volume_transport",
            Self::ColumnPhysics => "column_physics",
            Self::Observation => "observation",
        }
    }
}

pub(super) fn diagnostic_path() -> String {
    let _edge_count = EDGES.len();
    NODES
        .iter()
        .map(|node| node.name())
        .collect::<Vec<_>>()
        .join("->")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    #[test]
    fn mesoscale_graph_is_acyclic_and_physics_precedes_observation() {
        let mut indegree = NODES
            .into_iter()
            .map(|node| (node, 0usize))
            .collect::<HashMap<_, _>>();
        for (_, target) in EDGES {
            *indegree.get_mut(&target).expect("registered target") += 1;
        }
        let mut queue = indegree
            .iter()
            .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
            .collect::<VecDeque<_>>();
        let mut visited = Vec::new();
        while let Some(node) = queue.pop_front() {
            visited.push(node);
            for (source, target) in EDGES {
                if source == node {
                    let degree = indegree.get_mut(&target).expect("registered target");
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(target);
                    }
                }
            }
        }
        assert_eq!(visited.len(), NODES.len());
        assert_eq!(
            *visited.last().expect("nodes"),
            MesoscaleNodeId::Observation
        );
        let observation_index = visited
            .iter()
            .position(|node| *node == MesoscaleNodeId::Observation)
            .unwrap();
        assert!(visited[..observation_index]
            .iter()
            .all(|node| *node != MesoscaleNodeId::Observation));
    }

    #[test]
    fn mesoscale_physics_contract_contains_no_random_or_weather_preset_node() {
        let physical = NODES
            .iter()
            .take_while(|node| **node != MesoscaleNodeId::Observation)
            .map(|node| node.name())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!physical.contains("random"));
        assert!(!physical.contains("preset"));
        assert!(!physical.contains("weather"));
    }
}
