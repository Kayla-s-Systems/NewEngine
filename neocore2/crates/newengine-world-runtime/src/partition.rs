use newengine_world_api::{
    WorldCellCoord, WorldCellRecord, WorldCellResidency, WorldPartitionState,
};

use crate::service::EngineWorldGatewayService;

impl EngineWorldGatewayService {
    pub(crate) fn build_partition_cache(
        partition: &WorldPartitionState,
    ) -> (Vec<WorldCellRecord>, Vec<WorldCellCoord>) {
        if !partition.enabled {
            return (Vec::new(), Vec::new());
        }

        let radius = partition
            .simulation_radius
            .max(partition.render_radius)
            .clamp(0, 16);
        let side = (radius * 2 + 1) as usize;
        let mut cells = Vec::with_capacity(side.saturating_mul(side));

        for z in (partition.center.z - radius)..=(partition.center.z + radius) {
            for x in (partition.center.x - radius)..=(partition.center.x + radius) {
                let dx = (x - partition.center.x).abs();
                let dz = (z - partition.center.z).abs();
                let distance = dx.max(dz);
                let residency = match (
                    distance <= partition.render_radius,
                    distance <= partition.simulation_radius,
                ) {
                    (true, true) => WorldCellResidency::RenderAndSimulation,
                    (true, false) => WorldCellResidency::Render,
                    (false, true) => WorldCellResidency::Simulation,
                    (false, false) => continue,
                };

                cells.push(WorldCellRecord {
                    coord: WorldCellCoord { x, z },
                    residency,
                    dirty: false,
                    reason: "world partition desired residency".to_owned(),
                });
            }
        }

        cells.sort_unstable_by_key(|cell| distance_key(cell.coord, partition.center));
        let desired_cells = cells.iter().map(|cell| cell.coord).collect();
        (cells, desired_cells)
    }
}

#[inline]
pub(crate) fn distance_key(coord: WorldCellCoord, center: WorldCellCoord) -> (i64, i32, i32) {
    let dx = i64::from(coord.x) - i64::from(center.x);
    let dz = i64::from(coord.z) - i64::from(center.z);
    (dx * dx + dz * dz, coord.x, coord.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_cache_is_sorted_and_reuses_coords_for_plan() {
        let partition = WorldPartitionState {
            enabled: true,
            center: WorldCellCoord::new(10, -4),
            render_radius: 1,
            simulation_radius: 1,
            ..WorldPartitionState::default()
        };
        let (cells, desired) = EngineWorldGatewayService::build_partition_cache(&partition);

        assert_eq!(cells.len(), 9);
        assert_eq!(desired.len(), 9);
        assert_eq!(desired[0], partition.center);
        assert_eq!(
            desired,
            cells.iter().map(|cell| cell.coord).collect::<Vec<_>>()
        );
    }
}
