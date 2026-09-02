use std::sync::Arc;

use newengine_engine_runtime::{
    SceneBootstrapContext, SceneBootstrapProvider, SceneBootstrapResult,
};

/// Provider-resolved authored map bootstrap input. The generic provider owns startup-scene
/// selection and INDEX resolution; product/domain contributors may only enrich/assemble from
/// this already-resolved map identity.
#[derive(Clone, Debug)]
pub struct ResolvedAuthoredMapBootstrap {
    pub logical_path: String,
    pub map_ref: String,
    pub index: newengine_assets_api::MapIndexV1,
}

/// Ordered composition seam for domain-specific authored-world assembly. This is not a
/// SceneBootstrapProvider: provider identity, scene reset/foundation, startup-scene resolution
/// and map INDEX ownership remain in `AuthoredMapSceneBootstrapProvider`.
pub trait AuthoredMapSceneBootstrapContributor: Send + Sync {
    fn id(&self) -> &'static str;

    fn contribute(
        &self,
        ctx: &mut SceneBootstrapContext<'_>,
        map: &ResolvedAuthoredMapBootstrap,
    ) -> Result<SceneBootstrapResult, String>;
}

pub struct AuthoredMapSceneBootstrapProvider {
    contributors: Vec<Arc<dyn AuthoredMapSceneBootstrapContributor>>,
}

impl Default for AuthoredMapSceneBootstrapProvider {
    fn default() -> Self {
        Self {
            contributors: Vec::new(),
        }
    }
}

impl AuthoredMapSceneBootstrapProvider {
    #[inline]
    pub fn shared() -> Arc<dyn SceneBootstrapProvider> {
        Arc::new(Self::default())
    }

    #[inline]
    pub fn shared_with_contributor(
        contributor: Arc<dyn AuthoredMapSceneBootstrapContributor>,
    ) -> Arc<dyn SceneBootstrapProvider> {
        Self::shared_with_contributors(vec![contributor])
    }

    pub fn shared_with_contributors(
        contributors: Vec<Arc<dyn AuthoredMapSceneBootstrapContributor>>,
    ) -> Arc<dyn SceneBootstrapProvider> {
        Arc::new(Self { contributors })
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
        let (map_ref, index) = super::loader::load_authored_map_index(logical_path)?;
        let resolved = ResolvedAuthoredMapBootstrap {
            logical_path: logical_path.to_owned(),
            map_ref,
            index,
        };

        // Scene lifetime/foundation is generic bootstrap ownership. Domain contributors always
        // receive the same clean runtime scene and must never reset it independently.
        *ctx.scene = newengine_scene::Scene::new();
        newengine_engine_runtime::world_authoring::bootstrap_runtime_scene_foundation(ctx.scene);
        let root = newengine_engine_runtime::world_authoring::ensure_scene_root(ctx.scene);

        if !self.contributors.is_empty() {
            let mut primary = None;
            for contributor in &self.contributors {
                newengine_ulog_api::ulog::info!(
                    "authored-world bootstrap contributor begin contributor='{}' map='{}' cells={} provider='{}'",
                    contributor.id(),
                    resolved.map_ref,
                    resolved.index.cells.len(),
                    self.id(),
                );
                let result = contributor.contribute(ctx, &resolved)?;
                if result.primary_entity.is_some() {
                    primary = result.primary_entity;
                }
            }
            return Ok(SceneBootstrapResult::new(primary.or(Some(root))));
        }

        let map = super::loader::load_authored_map_from_index(
            resolved.map_ref.clone(),
            resolved.index.clone(),
        )?;
        let (primary, _stats) = super::materialize::materialize_map(ctx.scene, &map)?;
        Ok(SceneBootstrapResult::new(Some(primary)))
    }
}
