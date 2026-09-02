#![forbid(unsafe_op_in_unsafe_fn)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use newengine_ai_api::{
    ai_method, AiAgentSnapshotV1, AiFrameInputV1, AiFrameOutputV1, AiIntentDtoV1, AiIntentKind,
    AiPerceptionFactV1, ENGINE_AI_SERVICE_ID,
};
use newengine_ecs::{EntityId, World};
use newengine_entity_api::EntityHandle;
use newengine_math::Vec3;
use newengine_navigation_api::NavVec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_transform::Transform;

use super::{
    CharacterBody, CharacterControlState, CharacterLifeState, Health, PlayerStanceKind,
    PlayerStanceState,
};

#[path = "ai/types.rs"]
mod types;
use types::finite_non_negative;
pub use types::{
    AIController, AIPerceptionProbe, CombatIntent, CombatIntentKind, CombatTeam, PerceptionState,
    PerceptionTuning, TargetMemory,
};

#[inline]
fn nav_vec3(value: Vec3) -> NavVec3 {
    NavVec3::new(value.x, value.y, value.z)
}

#[path = "ai/perception.rs"]
mod perception;
use perception::{clear_ai_runtime_state, controller_is_operational, set_combat_intent};
pub use perception::{
    collect_ai_perception_queries, prepare_ai_perception, resolve_ai_perception_query_hits,
};

#[path = "ai/decision_bridge.rs"]
mod decision_bridge;
pub use decision_bridge::{apply_ai_frame_output, build_ai_frame_input, step_ai_decisions};

#[cfg(test)]
#[path = "ai/tests.rs"]
mod tests;
