#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "readiness/gate.rs"]
mod gate;
#[path = "readiness/materials.rs"]
mod materials;
#[path = "readiness/residency.rs"]
mod residency;
#[path = "readiness/status.rs"]
mod status;

pub(super) use gate::{
    prepare_scene_launch_resources, update_world_activation_gate,
    update_world_activation_gate_with_material_plan,
};
