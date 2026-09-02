#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::Resources;
use newengine_game_data::GameDataProvider;

/// Profile-owned GameData acquisition stage. The generic authored-world provider already owns
/// scene reset/foundation and map resolution; this contributor only publishes the immutable
/// project policy snapshot consumed by later domain contributors.
pub(crate) struct ProjectGameDataBootstrapContributor {
    game_data_provider: Arc<dyn GameDataProvider>,
}

impl ProjectGameDataBootstrapContributor {
    pub(crate) fn shared(
        game_data_provider: Arc<dyn GameDataProvider>,
    ) -> Arc<dyn newengine_authored_world_runtime::AuthoredMapSceneBootstrapContributor> {
        Arc::new(Self { game_data_provider })
    }
}

impl newengine_authored_world_runtime::AuthoredMapSceneBootstrapContributor
    for ProjectGameDataBootstrapContributor
{
    fn id(&self) -> &'static str {
        "project.game-data"
    }

    fn contribute(
        &self,
        ctx: &mut newengine_engine_runtime::SceneBootstrapContext<'_>,
        _resolved_map: &newengine_authored_world_runtime::ResolvedAuthoredMapBootstrap,
    ) -> Result<newengine_engine_runtime::SceneBootstrapResult, String> {
        let provider_id = self.game_data_provider.id();
        let snapshot = self
            .game_data_provider
            .load_snapshot()
            .map_err(|error| format!("game-data provider '{}' failed: {error}", provider_id))?;
        snapshot.data().validate().map_err(|error| {
            format!(
                "game-data provider '{}' produced invalid snapshot: {error}",
                provider_id
            )
        })?;
        ctx.scene.world_mut().insert_resource(snapshot);
        Ok(newengine_engine_runtime::SceneBootstrapResult::new(None))
    }
}

/// Transitional completion hook. The graph is project-authored; the generic authored-world
/// lifecycle invokes this without knowing anything about GameReady or audio policy.
pub(crate) struct ProjectAudioMixBootstrapCompletion;

impl newengine_authored_world_runtime::AuthoredWorldBootstrapCompletion
    for ProjectAudioMixBootstrapCompletion
{
    fn id(&self) -> &'static str {
        "project.audio-mix"
    }

    fn complete(
        &self,
        resources: &mut Resources,
        scene: &Arc<newengine_scene_runtime::SceneBridge>,
    ) -> Result<(), String> {
        let graph = {
            let scene = scene.scene();
            let scene = scene.read();
            let snapshot = scene
                .world()
                .resource::<newengine_game_data::GameDataSnapshot>()
                .ok_or(
                    "authored scene did not publish GameDataSnapshot before audio mix bootstrap",
                )?;
            snapshot.data().audio.mix_graph.clone()
        };
        graph
            .validate()
            .map_err(|error| format!("project audio.mix_graph invalid: {error}"))?;
        let handle = resources
            .get::<newengine_audio_world_runtime::AudioOrchestrationHandle>()
            .cloned()
            .ok_or("project audio.mix_graph requires AudioOrchestrationHandle")?;
        handle
            .install_mix_graph(graph.clone())
            .map_err(|error| format!("project audio.mix_graph installation failed: {error}"))?;
        newengine_ulog_api::ulog::info!(
            "project audio mix graph queued routes={} snapshots={} voice_budgets={} authority='project GameData -> AudioOrchestration'",
            graph.buses.len(),
            graph.snapshots.len(),
            graph.voice_budgets.len(),
        );
        Ok(())
    }
}
