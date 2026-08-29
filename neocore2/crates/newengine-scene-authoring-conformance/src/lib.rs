#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use newengine_engine_runtime::SceneBridge;
    use newengine_scene::Scene;
    use newengine_scene_authoring_runtime::SceneAuthoringRuntime;
    use newengine_transform::Transform;
    use newengine_world_authoring_api::{
        AuthoredMapPlacement, AuthoredMapPlacementDirty, AuthoredMapPlacementSource,
    };

    #[test]
    fn runtime_scene_selection_does_not_require_authoring_provider() {
        let bridge = SceneBridge::new(Scene::new());
        assert!(!bridge.scene_authoring_available());
        let entity = {
            let scene = bridge.scene();
            let mut scene = scene.write();
            scene.world_mut().spawn()
        };
        bridge.set_selection(Some(entity));
        assert_eq!(bridge.selection(), Some(entity));
    }

    #[test]
    fn authoring_provider_is_explicitly_replaceable() {
        let bridge = SceneBridge::new(Scene::new());
        bridge.set_scene_authoring_provider(Arc::new(SceneAuthoringRuntime::default()));
        assert!(bridge.scene_authoring_available());
        assert!(bridge.set_in_game_editor_enabled(true));
        assert!(bridge.in_game_editor_enabled());
        bridge.clear_scene_authoring_provider();
        assert!(!bridge.scene_authoring_available());
        assert!(!bridge.in_game_editor_enabled());
    }

    #[test]
    fn injected_provider_observes_authored_dirty_state_without_owning_scene() {
        let bridge = SceneBridge::new(Scene::new());
        bridge.set_scene_authoring_provider(Arc::new(SceneAuthoringRuntime::default()));
        {
            let scene = bridge.scene();
            let mut scene = scene.write();
            let entity = scene.world_mut().spawn();
            let _ = scene.world_mut().insert(entity, Transform::default());
            let _ = scene.world_mut().insert(
                entity,
                AuthoredMapPlacement::new(
                    "maps/test.ymap",
                    "oak",
                    AuthoredMapPlacementSource::ProfilePrefab,
                    true,
                ),
            );
            let _ = scene.world_mut().insert(entity, AuthoredMapPlacementDirty);
        }
        let status = bridge.authored_project_edit_status();
        assert_eq!(status.dirty_placements, 1);
        assert_eq!(status.pending_creates, 0);
        assert_eq!(status.pending_deletes, 0);
    }
}
