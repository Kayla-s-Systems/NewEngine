use newengine_world_api::{WorldBootPhase, WorldCellCoord, WorldCellRecord, WorldPartitionState};

#[derive(Debug)]
pub(crate) struct WorldRuntimeBookkeeping {
    pub(crate) world_instance_id: String,
    pub(crate) phase: WorldBootPhase,
    pub(crate) deterministic: bool,
    pub(crate) boot_sequence: u64,
    pub(crate) partition: WorldPartitionState,
    pub(crate) active_cells: Vec<WorldCellRecord>,
    pub(crate) desired_cells: Vec<WorldCellCoord>,
    pub(crate) notes: Vec<String>,
}

impl Default for WorldRuntimeBookkeeping {
    #[inline]
    fn default() -> Self {
        Self {
            world_instance_id: "world.runtime.default".to_owned(),
            phase: WorldBootPhase::Cold,
            deterministic: true,
            boot_sequence: 0,
            partition: WorldPartitionState::default(),
            active_cells: Vec::new(),
            desired_cells: Vec::new(),
            notes: vec![
                "Scene is authored structure; World is living runtime instance.".to_owned(),
                "ECS remains storage behind engine.ecs; native EntityId is not exposed.".to_owned(),
            ],
        }
    }
}
