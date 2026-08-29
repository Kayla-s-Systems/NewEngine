use std::collections::{BTreeMap, BTreeSet};

use newengine_world_api::WorldCellCoord;
use newengine_world_environment_api::EnvironmentSurfaceBoundaryDto;

#[derive(Clone, Debug)]
pub(super) struct GridTopology {
    pub surfaces: Vec<EnvironmentSurfaceBoundaryDto>,
    pub index_by_cell: BTreeMap<WorldCellCoord, usize>,
    /// Each internal cardinal face appears exactly once: (left/south, right/north, dx, dz).
    pub faces: Vec<(usize, usize, i32, i32)>,
    pub duplicate_boundaries: usize,
}

pub(super) fn build(boundaries: &[EnvironmentSurfaceBoundaryDto]) -> GridTopology {
    let mut seen = BTreeSet::new();
    let mut duplicate_boundaries = 0usize;
    let mut surfaces = boundaries
        .iter()
        .copied()
        .filter(|surface| {
            if seen.insert(surface.cell) {
                true
            } else {
                duplicate_boundaries += 1;
                false
            }
        })
        .collect::<Vec<_>>();
    surfaces.sort_by_key(|surface| surface.cell);

    let index_by_cell = surfaces
        .iter()
        .enumerate()
        .map(|(index, surface)| (surface.cell, index))
        .collect::<BTreeMap<_, _>>();
    let mut faces = Vec::new();
    for (index, surface) in surfaces.iter().enumerate() {
        for (dx, dz) in [(1, 0), (0, 1)] {
            let neighbor = WorldCellCoord::new(surface.cell.x + dx, surface.cell.z + dz);
            if let Some(&other) = index_by_cell.get(&neighbor) {
                faces.push((index, other, dx, dz));
            }
        }
    }
    GridTopology {
        surfaces,
        index_by_cell,
        faces,
        duplicate_boundaries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(x: i32, z: i32) -> EnvironmentSurfaceBoundaryDto {
        EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(x, z),
            ..EnvironmentSurfaceBoundaryDto::default()
        }
    }

    #[test]
    fn topology_contains_each_internal_cardinal_face_once() {
        let grid = build(&[surface(0, 0), surface(1, 0), surface(0, 1), surface(1, 1)]);
        assert_eq!(grid.surfaces.len(), 4);
        assert_eq!(grid.faces.len(), 4);
        assert_eq!(grid.duplicate_boundaries, 0);
    }

    #[test]
    fn duplicate_world_boundaries_do_not_create_duplicate_atmosphere_cells() {
        let grid = build(&[surface(0, 0), surface(0, 0), surface(1, 0)]);
        assert_eq!(grid.surfaces.len(), 2);
        assert_eq!(grid.duplicate_boundaries, 1);
    }
}
