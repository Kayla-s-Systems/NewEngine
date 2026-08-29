#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral authoring control-plane contract.
//! The contract knows authored identity + ECS world access, but no concrete scene bridge,
//! renderer, gameplay, editor viewport implementation, package format, or Host.

use std::path::Path;

use newengine_ecs::{EntityId, World};
use newengine_world_authoring_api::{AuthoredMapPlacement, AuthoredMapPlacementCloneSource};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthoredProjectEditStatus {
    pub dirty_placements: usize,
    pub pending_creates: usize,
    pub pending_deletes: usize,
    pub last_save_succeeded: Option<bool>,
    pub last_save_message: String,
}

pub trait SceneAuthoringService: Send + Sync {
    fn in_game_editor_enabled(&self) -> bool;
    fn set_in_game_editor_enabled(&self, enabled: bool) -> bool;

    fn save_authored_project_world(
        &self,
        world: &mut World,
        project_root: Option<&Path>,
    ) -> Result<usize, String>;

    fn authored_project_edit_status(&self, world: &World) -> AuthoredProjectEditStatus;

    fn prepare_authored_duplicate(
        &self,
        world: &World,
        source_entity: EntityId,
        source: &AuthoredMapPlacement,
    ) -> Option<(AuthoredMapPlacement, AuthoredMapPlacementCloneSource)>;

    fn record_authored_deletion(&self, authored: &AuthoredMapPlacement);
}
