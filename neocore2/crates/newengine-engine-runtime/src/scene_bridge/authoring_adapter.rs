use std::path::PathBuf;

use newengine_ecs::{EntityId, World};
use newengine_world_authoring_api::{AuthoredMapPlacement, AuthoredMapPlacementCloneSource};

use super::SceneBridge;

impl SceneBridge {
    pub fn save_authored_project_world(&self) -> Result<usize, String> {
        let project_root = crate::env_config::var_os("NEWENGINE_PROJECT_ROOT").map(PathBuf::from);
        let authoring = self
            .scene_authoring_provider()
            .ok_or_else(|| "scene authoring provider is unavailable".to_owned())?;
        let mut scene = self.scene.write();
        authoring.save_authored_project_world(scene.world_mut(), project_root.as_deref())
    }

    pub fn authored_project_edit_status(
        &self,
    ) -> newengine_scene_authoring_api::AuthoredProjectEditStatus {
        let Some(authoring) = self.scene_authoring_provider() else {
            return newengine_scene_authoring_api::AuthoredProjectEditStatus::default();
        };
        let scene = self.scene.read();
        authoring.authored_project_edit_status(scene.world())
    }

    pub(super) fn prepare_authored_duplicate(
        &self,
        world: &World,
        source_entity: EntityId,
        source: &AuthoredMapPlacement,
    ) -> Option<(AuthoredMapPlacement, AuthoredMapPlacementCloneSource)> {
        self.scene_authoring_provider()?
            .prepare_authored_duplicate(world, source_entity, source)
    }

    pub(super) fn record_authored_deletion(&self, authored: &AuthoredMapPlacement) {
        if let Some(authoring) = self.scene_authoring_provider() {
            authoring.record_authored_deletion(authored);
        }
    }
}
