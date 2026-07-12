use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WorldBootPhase {
    #[default]
    Cold,
    SceneDeclared,
    RuntimeBootstrapped,
    LaunchGated,
    Playable,
    Headless,
    Shutdown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct WorldCellCoord {
    pub x: i32,
    pub z: i32,
}

impl WorldCellCoord {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WorldCellResidency {
    #[default]
    Unloaded,
    Loading,
    Simulation,
    Render,
    RenderAndSimulation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldCellRecord {
    pub coord: WorldCellCoord,
    pub residency: WorldCellResidency,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorldPartitionState {
    pub enabled: bool,
    pub cell_size_x: u32,
    pub cell_size_z: u32,
    pub center: WorldCellCoord,
    #[serde(default)]
    pub render_radius: i32,
    #[serde(default)]
    pub simulation_radius: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorldStreamingCellsRequest {
    #[serde(default)]
    pub include_unloaded: bool,
    #[serde(default)]
    pub include_reasons: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldStreamingCellDto {
    pub coord: WorldCellCoord,
    pub residency: WorldCellResidency,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldStreamingPlanDto {
    pub center: WorldCellCoord,
    pub render_radius: i32,
    pub simulation_radius: i32,
    #[serde(default)]
    pub desired_cells: Vec<WorldCellCoord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldStreamingCellsResponse {
    pub partition: WorldPartitionState,
    pub plan: WorldStreamingPlanDto,
    #[serde(default)]
    pub cells: Vec<WorldStreamingCellDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_constructor_is_const_shape() {
        assert_eq!(WorldCellCoord::new(4, -3), WorldCellCoord { x: 4, z: -3 });
    }
}
