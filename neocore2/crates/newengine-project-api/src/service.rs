use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{ContentMountDescriptor, ProjectPathRole, RuntimeLaunchProfile};

/// Host-owned project authority for the currently selected project/runtime.
pub const ENGINE_PROJECT_SERVICE_ID: &str = "engine.project";
pub const ENGINE_PROJECT_SERVICE_SCHEMA_V1: &str = "newengine.project.service.v1";

pub mod method {
    pub const SELECTED_V1: &str = "project.selected_v1";
    pub const RESOLVE_PATH_V1: &str = "project.resolve_path_v1";
    pub const RESOLVE_AUTHORED_V1: &str = "project.resolve_authored_v1";
    pub const CONTENT_MOUNTS_V1: &str = "project.content_mounts_v1";
    pub const METADATA_V1: &str = "project.metadata_v1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedProjectV1 {
    pub id: String,
    pub name: String,
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub launch_id: String,
    pub launch_profile: RuntimeLaunchProfile,
    pub runtime_profile: Option<String>,
    pub game_module: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveProjectPathRequestV1 {
    pub role: ProjectPathRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveAuthoredPathRequestV1 {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProjectPathV1 {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContentMountsV1 {
    pub mounts: Vec<ContentMountDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadataV1 {
    pub id: String,
    pub name: String,
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub startup_scene: Option<String>,
    pub definitions: Vec<PathBuf>,
    pub scripting_runtime: Option<String>,
}
