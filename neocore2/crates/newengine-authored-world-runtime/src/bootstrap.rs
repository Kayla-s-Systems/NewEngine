use std::sync::Arc;

use newengine_engine_runtime::{
    SceneBootstrapContext, SceneBootstrapProvider, SceneBootstrapResult,
};

pub struct AuthoredMapSceneBootstrapProvider;

impl AuthoredMapSceneBootstrapProvider {
    #[inline]
    pub fn shared() -> Arc<dyn SceneBootstrapProvider> {
        Arc::new(Self)
    }
}

impl SceneBootstrapProvider for AuthoredMapSceneBootstrapProvider {
    fn id(&self) -> &'static str {
        "engine.authored-world.ymap-bootstrap"
    }

    fn bootstrap(
        &self,
        ctx: &mut SceneBootstrapContext<'_>,
    ) -> Result<SceneBootstrapResult, String> {
        let logical_path = newengine_plugin_host::current_host_context()
            .environment_var(newengine_project_api::PROJECT_STARTUP_SCENE_ENV)
            .ok_or_else(|| "authored-world bootstrap requires project startup_scene".to_owned())?;
        let logical_path = logical_path.trim();
        if logical_path.is_empty() {
            return Err("authored-world bootstrap received an empty startup_scene".to_owned());
        }
        let map = super::loader::load_authored_map(logical_path)?;
        let (primary, _stats) = super::materialize::materialize_map(ctx.scene, &map)?;
        Ok(SceneBootstrapResult::new(Some(primary)))
    }
}
