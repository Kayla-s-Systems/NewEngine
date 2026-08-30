#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_project_api::{
    method, ProjectContentMountsV1, ProjectMetadataV1, ResolveAuthoredPathRequestV1,
    ResolveProjectPathRequestV1, ResolvedProjectPathV1, SelectedProjectV1,
    ENGINE_PROJECT_SERVICE_ID, ENGINE_PROJECT_SERVICE_SCHEMA_V1,
};
use newengine_project_runtime::ProjectRuntimeContext;
use newengine_service_kit::JsonServiceRouter;

#[derive(Clone)]
struct ProjectServiceState {
    project: ProjectRuntimeContext,
}

fn selected(state: &mut ProjectServiceState) -> SelectedProjectV1 {
    SelectedProjectV1 {
        id: state.project.manifest.id.clone(),
        name: state.project.manifest.name.clone(),
        project_root: state.project.project_root.clone(),
        manifest_path: state.project.manifest_path.clone(),
        launch_id: state.project.launch.preset_id.clone(),
        launch_profile: state.project.launch.profile,
        runtime_profile: state.project.launch.runtime_profile.clone(),
        game_module: state.project.manifest.game_module.clone(),
    }
}

fn resolve_path(
    state: &mut ProjectServiceState,
    request: ResolveProjectPathRequestV1,
) -> ResolvedProjectPathV1 {
    ResolvedProjectPathV1 {
        path: state.project.paths().resolve(request.role),
    }
}

fn resolve_authored(
    state: &mut ProjectServiceState,
    request: ResolveAuthoredPathRequestV1,
) -> ResolvedProjectPathV1 {
    ResolvedProjectPathV1 {
        path: state.project.paths().resolve_authored(&request.path),
    }
}

fn content_mounts(state: &mut ProjectServiceState) -> ProjectContentMountsV1 {
    ProjectContentMountsV1 {
        mounts: state.project.mounts.mounts().to_vec(),
    }
}

fn metadata(state: &mut ProjectServiceState) -> ProjectMetadataV1 {
    ProjectMetadataV1 {
        id: state.project.manifest.id.clone(),
        name: state.project.manifest.name.clone(),
        manifest_path: state.project.manifest_path.clone(),
        project_root: state.project.project_root.clone(),
        startup_scene: state.project.launch.startup_scene.clone(),
        definitions: state.project.manifest.definitions.clone(),
        scripting_runtime: state.project.scripts.runtime().map(str::to_owned),
    }
}

fn service(project: ProjectRuntimeContext) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = serde_json::json!({
        "schema": ENGINE_PROJECT_SERVICE_SCHEMA_V1,
        "service_id": ENGINE_PROJECT_SERVICE_ID,
        "owner": "newengine-runtime-host",
        "authority": "selected-project",
        "methods": [
            method::SELECTED_V1,
            method::RESOLVE_PATH_V1,
            method::RESOLVE_AUTHORED_V1,
            method::CONTENT_MOUNTS_V1,
            method::METADATA_V1,
        ],
        "notes": "Host-owned immutable authority for the project selected before runtime composition."
    });

    JsonServiceRouter::with_state(ENGINE_PROJECT_SERVICE_ID, ProjectServiceState { project })
        .describe_json(&description)
        .get_json(method::SELECTED_V1, selected)
        .post_json::<ResolveProjectPathRequestV1, ResolvedProjectPathV1, _>(
            method::RESOLVE_PATH_V1,
            resolve_path,
        )
        .post_json::<ResolveAuthoredPathRequestV1, ResolvedProjectPathV1, _>(
            method::RESOLVE_AUTHORED_V1,
            resolve_authored,
        )
        .get_json(method::CONTENT_MOUNTS_V1, content_mounts)
        .get_json(method::METADATA_V1, metadata)
        .into_service_v1()
}

pub(crate) fn register_selected_project_service(
    host: &newengine_plugin_host::HostContextHandle,
    project: &ProjectRuntimeContext,
) -> Result<(), String> {
    newengine_plugin_host::with_host_context(host, || {
        newengine_plugin_host::host_register_service_impl(service(project.clone())).into_result()
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_project_api::{ProjectManifest, ProjectPathRole, ResolvedProjectLaunch};
    use std::path::PathBuf;

    fn context() -> ProjectRuntimeContext {
        ProjectRuntimeContext {
            manifest_path: PathBuf::from("project/game.toml"),
            project_root: PathBuf::from("project"),
            manifest: ProjectManifest {
                id: "sample".to_owned(),
                name: "Sample".to_owned(),
                ..ProjectManifest::default()
            },
            launch: ResolvedProjectLaunch {
                preset_id: "game".to_owned(),
                profile: newengine_project_api::RuntimeLaunchProfile::Game,
                runtime_profile: None,
                startup_scene: None,
                startup_presentation_state: None,
            },
            mounts: Default::default(),
            scripts: Default::default(),
        }
    }

    #[test]
    fn semantic_path_resolution_uses_project_filesystem_authority() {
        let mut state = ProjectServiceState { project: context() };
        let resolved = resolve_path(
            &mut state,
            ResolveProjectPathRequestV1 {
                role: ProjectPathRole::Source,
            },
        );
        assert_eq!(
            resolved.path,
            PathBuf::from("project").join(newengine_project_api::PROJECT_SOURCE_DIR)
        );
    }
}
