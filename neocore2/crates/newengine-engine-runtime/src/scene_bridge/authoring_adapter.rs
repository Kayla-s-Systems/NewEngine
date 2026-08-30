use newengine_ecs::{EntityId, World};
use newengine_plugin_api::Blob;
use newengine_project_api::{
    method as project_method, SelectedProjectV1, ENGINE_PROJECT_SERVICE_ID,
};
use newengine_world_authoring_api::{AuthoredMapPlacement, AuthoredMapPlacementCloneSource};

use super::SceneBridge;

fn selected_project_root() -> Option<std::path::PathBuf> {
    let response = newengine_plugin_host::call_service_v1(
        ENGINE_PROJECT_SERVICE_ID.into(),
        project_method::SELECTED_V1.into(),
        Blob::from(Vec::new()),
    )
    .into_result()
    .ok()?;
    serde_json::from_slice::<SelectedProjectV1>(response.as_slice())
        .ok()
        .map(|project| project.project_root)
}

impl SceneBridge {
    pub fn save_authored_project_world(&self) -> Result<usize, String> {
        let project_root = selected_project_root();
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
