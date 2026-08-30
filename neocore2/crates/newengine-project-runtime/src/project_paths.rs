// Project filesystem vocabulary is API-owned so runtime, authoring and tooling
// depend on the same semantic contract without depending on each other.
pub use newengine_project_api::{
    normalize_project_manifest_request, ProjectFilesystem as ProjectPaths, ProjectPathRole,
    PROJECT_ASSET_BUILD_PLAN_FILE, PROJECT_PLUGINS_DIR as CONVENTIONAL_PROJECT_PLUGINS_DIR,
    PROJECT_SOURCE_DIR,
};
