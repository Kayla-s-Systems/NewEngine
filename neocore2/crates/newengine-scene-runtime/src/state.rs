use std::sync::Arc;

use newengine_asset_bootstrap_runtime::ProfileMountSpec;
use newengine_scene::{SceneAsset, SceneAssetOptions};

use crate::SceneBridge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneGatewayAssetMounts {
    pub profile: ProfileMountSpec,
}

impl SceneGatewayAssetMounts {
    #[inline]
    pub const fn from_profile(profile: ProfileMountSpec) -> Self {
        Self { profile }
    }
}

#[derive(Clone)]
pub struct EngineSceneGatewayService {
    pub(crate) scene: Arc<SceneBridge>,
    pub(crate) asset_mounts: Option<SceneGatewayAssetMounts>,
}

impl EngineSceneGatewayService {
    #[inline]
    pub fn new(scene: Arc<SceneBridge>) -> Self {
        Self {
            scene,
            asset_mounts: None,
        }
    }

    #[inline]
    pub fn with_asset_mounts(
        scene: Arc<SceneBridge>,
        asset_mounts: SceneGatewayAssetMounts,
    ) -> Self {
        Self {
            scene,
            asset_mounts: Some(asset_mounts),
        }
    }

    pub(crate) fn authority_json(&self) -> serde_json::Value {
        let snapshot = self.scene.authority_snapshot();
        serde_json::json!({
            "authority": snapshot.authority_label(),
            "split": snapshot.has_split_world_authority(),
            "ecs_owner": snapshot.ecs.as_ref().map(|route| route.provider_owner_id.clone()),
            "entity_owner": snapshot.entity.as_ref().map(|route| route.provider_owner_id.clone()),
            "scene_owner": snapshot.scene.as_ref().map(|route| route.provider_owner_id.clone()),
            "notes": snapshot.notes.clone(),
        })
    }

    pub(crate) fn current_scene_asset(&self, include_empty_entities: bool) -> SceneAsset {
        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        scene.to_asset(SceneAssetOptions {
            include_empty_entities,
        })
    }
}
