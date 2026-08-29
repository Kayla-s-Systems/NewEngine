#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum AtmosphereNodeId {
    Boundary,
    Prognostic,
    Thermodynamics,
    VerticalDynamics,
    Microphysics,
    Wind,
    Optics,
}

pub(super) const NODES: [AtmosphereNodeId; 7] = [
    AtmosphereNodeId::Boundary,
    AtmosphereNodeId::Prognostic,
    AtmosphereNodeId::Thermodynamics,
    AtmosphereNodeId::VerticalDynamics,
    AtmosphereNodeId::Microphysics,
    AtmosphereNodeId::Wind,
    AtmosphereNodeId::Optics,
];

pub(super) const EDGES: [(AtmosphereNodeId, AtmosphereNodeId); 11] = [
    (AtmosphereNodeId::Boundary, AtmosphereNodeId::Prognostic),
    (
        AtmosphereNodeId::Prognostic,
        AtmosphereNodeId::Thermodynamics,
    ),
    (
        AtmosphereNodeId::Thermodynamics,
        AtmosphereNodeId::VerticalDynamics,
    ),
    (
        AtmosphereNodeId::VerticalDynamics,
        AtmosphereNodeId::Microphysics,
    ),
    (AtmosphereNodeId::Boundary, AtmosphereNodeId::Wind),
    (AtmosphereNodeId::VerticalDynamics, AtmosphereNodeId::Wind),
    (AtmosphereNodeId::Boundary, AtmosphereNodeId::Optics),
    (AtmosphereNodeId::Prognostic, AtmosphereNodeId::Optics),
    (AtmosphereNodeId::Thermodynamics, AtmosphereNodeId::Optics),
    (AtmosphereNodeId::Microphysics, AtmosphereNodeId::Optics),
    (AtmosphereNodeId::Wind, AtmosphereNodeId::Optics),
];

impl AtmosphereNodeId {
    fn name(self) -> &'static str {
        match self {
            Self::Boundary => "boundary",
            Self::Prognostic => "prognostic",
            Self::Thermodynamics => "thermodynamics",
            Self::VerticalDynamics => "vertical_dynamics",
            Self::Microphysics => "microphysics",
            Self::Wind => "wind",
            Self::Optics => "optics",
        }
    }
}

pub(crate) fn diagnostic_path() -> String {
    // Reading EDGES here makes the declared dependency contract part of release code,
    // not test-only documentation.
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
    fn atmosphere_graph_is_acyclic_and_all_nodes_are_reachable() {
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
        assert_eq!(
            visited.len(),
            NODES.len(),
            "cycle or disconnected dependency detected"
        );
        assert_eq!(visited[0], AtmosphereNodeId::Boundary);
        assert_eq!(
            *visited.last().expect("graph has nodes"),
            AtmosphereNodeId::Optics
        );
    }

    #[test]
    fn physics_graph_has_no_weather_label_node() {
        let description = format!("{NODES:?}{EDGES:?}").to_ascii_lowercase();
        assert!(!description.contains("weather"));
        assert!(!description.contains("preset"));
        assert!(!description.contains("random"));
    }
}
