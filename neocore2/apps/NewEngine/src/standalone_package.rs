use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use newengine_project_runtime::ProjectRuntimeContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandaloneTargetOs {
    Windows,
    Linux,
    MacOs,
}

impl StandaloneTargetOs {
    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "windows" | "win" | "win32" | "win64" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            "macos" | "mac" | "osx" => Some(Self::MacOs),
            _ => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "macos",
        }
    }

    fn ensure_supported(self) -> Result<(), String> {
        match self {
            Self::Windows if cfg!(windows) => Ok(()),
            Self::Linux if cfg!(target_os = "linux") => Ok(()),
            Self::MacOs if cfg!(target_os = "macos") => Ok(()),
            _ => Err(format!(
                "standalone target '{}' requires a native build pipeline on that host OS",
                self.id()
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StandaloneBuildOptions {
    pub output_dir: Option<PathBuf>,
    pub package_name: Option<String>,
    pub target_os: StandaloneTargetOs,
    pub rebuild_plugins: bool,
    pub include_source: bool,
}

impl Default for StandaloneBuildOptions {
    fn default() -> Self {
        Self {
            output_dir: None,
            package_name: None,
            target_os: StandaloneTargetOs::Windows,
            rebuild_plugins: true,
            include_source: true,
        }
    }
}

/// Builds a self-contained Game Ready package around the generic NewEngine launcher.
///
/// The package deliberately keeps `game.toml` adjacent to `NewEngine.exe`, because the
/// launcher/runtime contract resolves an adjacent manifest for installed builds. Project-local
/// content is copied into the package root. External authored content mounts are materialized
/// under `ExternalContent/` and their roots are rewritten in the packaged manifest.
pub fn build_game_ready_standalone_with_options(
    runtime_config_path: &Path,
    project: &ProjectRuntimeContext,
    options: &StandaloneBuildOptions,
) -> Result<PathBuf, String> {
    options.target_os.ensure_supported()?;

    let workspace_root = runtime_config_path.parent().ok_or_else(|| {
        format!(
            "runtime config '{}' has no parent workspace",
            runtime_config_path.display()
        )
    })?;
    let newengine_root = workspace_root.parent().ok_or_else(|| {
        format!(
            "cannot resolve NewEngine root from '{}'",
            workspace_root.display()
        )
    })?;
    let northstar_root = newengine_root.parent().ok_or_else(|| {
        format!(
            "cannot resolve NorthStar root from '{}'",
            newengine_root.display()
        )
    })?;

    let package_name = sanitize_package_name(
        options
            .package_name
            .as_deref()
            .unwrap_or(project.manifest.id.as_str()),
    )?;
    let output_parent = options.output_dir.clone().unwrap_or_else(|| {
        project
            .project_root
            .parent()
            .and_then(Path::parent)
            .map(|root| root.join("Build"))
            .unwrap_or_else(|| project.project_root.join("Build"))
    });
    let output_root = absolutize(&output_parent)?.join(&package_name);

    let project_abs = canonical_or_absolute(&project.project_root)?;
    let output_abs = absolutize(&output_root)?;
    if output_abs == project_abs || output_abs.starts_with(&project_abs) {
        return Err(format!(
            "standalone output '{}' must be outside project source root '{}'",
            output_abs.display(),
            project_abs.display()
        ));
    }

    require_file(runtime_config_path, "runtime.toml")?;
    require_file(&workspace_root.join("config.json"), "config.json")?;
    require_dir(&northstar_root.join("pluginsRuntime"), "pluginsRuntime")?;
    require_file(&project.manifest_path, "game.toml")?;

    progress(0.03, "Preparing standalone build");
    if options.rebuild_plugins {
        rebuild_plugins(northstar_root)?;
    }

    progress(0.12, "Building NewEngine release");
    build_release_launcher(workspace_root)?;

    let source_exe = workspace_root
        .join("target")
        .join("release")
        .join(if cfg!(windows) {
            "NewEngine.exe"
        } else {
            "NewEngine"
        });
    require_file(&source_exe, "release NewEngine executable")?;

    if output_root.exists() {
        fs::remove_dir_all(&output_root).map_err(|error| {
            format!(
                "remove previous standalone output '{}': {error}",
                output_root.display()
            )
        })?;
    }
    fs::create_dir_all(&output_root).map_err(|error| {
        format!(
            "create standalone output '{}': {error}",
            output_root.display()
        )
    })?;

    progress(0.28, "Copying runtime");
    copy_file(
        &source_exe,
        &output_root.join(source_exe.file_name().unwrap()),
    )?;
    copy_file(runtime_config_path, &output_root.join("runtime.toml"))?;
    copy_file(
        &workspace_root.join("config.json"),
        &output_root.join("config.json"),
    )?;

    let optional_bink = newengine_root.join("bink2w64.dll");
    if optional_bink.is_file() {
        copy_file(&optional_bink, &output_root.join("bink2w64.dll"))?;
    }

    progress(0.42, "Copying runtime plugins");
    copy_tree_all(
        &northstar_root.join("pluginsRuntime"),
        &output_root.join("pluginsRuntime"),
    )?;

    progress(0.60, "Copying project content");
    copy_project_tree(&project.project_root, &output_root, options.include_source)?;

    progress(0.74, "Localizing external content");
    let source_manifest_text = fs::read_to_string(&project.manifest_path).map_err(|error| {
        format!(
            "read project manifest '{}': {error}",
            project.manifest_path.display()
        )
    })?;
    let mut manifest: toml::Value = toml::from_str(&source_manifest_text).map_err(|error| {
        format!(
            "parse project manifest '{}' for standalone packaging: {error}",
            project.manifest_path.display()
        )
    })?;
    localize_content_mounts(&mut manifest, project, &output_root)?;
    let packaged_manifest = toml::to_string_pretty(&manifest)
        .map_err(|error| format!("encode packaged game.toml: {error}"))?;
    fs::write(output_root.join("game.toml"), packaged_manifest)
        .map_err(|error| format!("write packaged game.toml: {error}"))?;

    progress(0.88, "Writing package manifest");
    let package_manifest = serde_json::json!({
        "schema": "northstar.standalone-package.v1",
        "created_unix_ms": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default(),
        "project": {
            "id": &project.manifest.id,
            "name": &project.manifest.name,
            "launch": &project.launch.preset_id,
            "runtime_profile": &project.launch.runtime_profile,
        },
        "target_os": options.target_os.id(),
        "launcher": source_exe.file_name().and_then(|value| value.to_str()).unwrap_or("NewEngine"),
        "runtime_config": "runtime.toml",
        "game_manifest": "game.toml",
        "plugins": "pluginsRuntime",
        "include_source": options.include_source,
    });
    fs::write(
        output_root.join("package-manifest.json"),
        serde_json::to_vec_pretty(&package_manifest)
            .map_err(|error| format!("encode standalone package manifest: {error}"))?,
    )
    .map_err(|error| format!("write standalone package manifest: {error}"))?;

    validate_package(&output_root, source_exe.file_name().unwrap())?;
    progress(1.0, "Standalone package ready");
    Ok(output_root)
}

fn rebuild_plugins(northstar_root: &Path) -> Result<(), String> {
    let script = northstar_root
        .join("PluginsSrc")
        .join("build_all_plugins.cmd");
    require_file(&script, "plugin release build script")?;
    progress(0.06, "Rebuilding runtime plugins");
    let status = Command::new("cmd.exe")
        .args(["/D", "/C"])
        .arg(&script)
        .current_dir(script.parent().unwrap_or(northstar_root))
        .status()
        .map_err(|error| format!("start plugin release build '{}': {error}", script.display()))?;
    if !status.success() {
        return Err(format!(
            "plugin release build '{}' exited with {status}",
            script.display()
        ));
    }
    Ok(())
}

fn build_release_launcher(workspace_root: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .current_dir(workspace_root)
        .args(["build", "-p", "newengine", "--release"])
        .status()
        .map_err(|error| format!("start NewEngine release build: {error}"))?;
    if !status.success() {
        return Err(format!("NewEngine release build exited with {status}"));
    }
    Ok(())
}

fn localize_content_mounts(
    manifest: &mut toml::Value,
    project: &ProjectRuntimeContext,
    output_root: &Path,
) -> Result<(), String> {
    let Some(content) = manifest
        .get_mut("content")
        .and_then(toml::Value::as_array_mut)
    else {
        return Ok(());
    };
    let project_root = canonical_or_absolute(&project.project_root)?;

    for (index, mount) in content.iter_mut().enumerate() {
        let Some(table) = mount.as_table_mut() else {
            return Err(format!("game.toml content[{index}] must be a table"));
        };
        let Some(authored_root) = table
            .get("root")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let source = {
            let root = PathBuf::from(authored_root);
            if root.is_absolute() {
                root
            } else {
                project.project_root.join(root)
            }
        };
        let required = table
            .get("required")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        if !source.exists() {
            if required {
                return Err(format!(
                    "required content mount '{}' does not exist",
                    source.display()
                ));
            }
            continue;
        }

        let source_abs = canonical_or_absolute(&source)?;
        if let Ok(relative) = source_abs.strip_prefix(&project_root) {
            let normalized = path_to_manifest(relative);
            table.insert("root".to_owned(), toml::Value::String(normalized));
            continue;
        }

        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .map(sanitize_mount_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("mount-{index}"));
        let destination_relative = PathBuf::from("ExternalContent").join(id);
        let destination = output_root.join(&destination_relative);
        if source_abs.is_dir() {
            copy_tree_all(&source_abs, &destination)?;
        } else {
            fs::create_dir_all(&destination)
                .map_err(|error| format!("create '{}': {error}", destination.display()))?;
            copy_file(
                &source_abs,
                &destination.join(source_abs.file_name().ok_or_else(|| {
                    format!(
                        "external content '{}' has no filename",
                        source_abs.display()
                    )
                })?),
            )?;
        }
        table.insert(
            "root".to_owned(),
            toml::Value::String(path_to_manifest(&destination_relative)),
        );
    }
    Ok(())
}

fn should_copy_project_path(relative: &Path, include_source: bool) -> bool {
    let mut components = relative.components();
    let first = components.next().and_then(|component| match component {
        Component::Normal(value) => value.to_str(),
        _ => None,
    });
    if first.is_some_and(|value| {
        value.eq_ignore_ascii_case(".git")
            || value.eq_ignore_ascii_case("target")
            || value.eq_ignore_ascii_case("Build")
            || value.eq_ignore_ascii_case(".idea")
    }) {
        return false;
    }
    if !include_source && first.is_some_and(|value| value.eq_ignore_ascii_case("Source")) {
        return false;
    }
    true
}

fn copy_tree_all(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "copy source '{}' is not a directory",
            source.display()
        ));
    }
    for entry in fs::read_dir(source)
        .map_err(|error| format!("read directory '{}': {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("read file type '{}': {error}", source_path.display()))?;
        if file_type.is_dir() {
            copy_tree_all(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            copy_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn copy_project_tree(
    source: &Path,
    destination: &Path,
    include_source: bool,
) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "project copy source '{}' is not a directory",
            source.display()
        ));
    }
    for entry in fs::read_dir(source)
        .map_err(|error| format!("read project directory '{}': {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("read project directory entry: {error}"))?;
        let source_path = entry.path();
        let relative = PathBuf::from(entry.file_name());
        if !should_copy_project_path(&relative, include_source) {
            continue;
        }
        let destination_path = destination.join(&relative);
        let file_type = entry
            .file_type()
            .map_err(|error| format!("read file type '{}': {error}", source_path.display()))?;
        if file_type.is_dir() {
            copy_tree_all(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            copy_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create directory '{}': {error}", parent.display()))?;
    }
    fs::copy(source, destination).map_err(|error| {
        format!(
            "copy '{}' -> '{}': {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn validate_package(output_root: &Path, executable_name: &std::ffi::OsStr) -> Result<(), String> {
    for relative in [
        PathBuf::from(executable_name),
        PathBuf::from("runtime.toml"),
        PathBuf::from("config.json"),
        PathBuf::from("game.toml"),
        PathBuf::from("package-manifest.json"),
    ] {
        require_file(&output_root.join(relative), "standalone package file")?;
    }
    require_dir(
        &output_root.join("pluginsRuntime"),
        "standalone pluginsRuntime",
    )?;
    Ok(())
}

fn sanitize_package_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("standalone package name must not be empty".to_owned());
    }
    if Path::new(trimmed).components().count() != 1
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(format!(
            "standalone package name '{trimmed}' must be a single safe directory name"
        ));
    }
    Ok(trimmed.to_owned())
}

fn sanitize_mount_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn path_to_manifest(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn canonical_or_absolute(path: &Path) -> Result<PathBuf, String> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(_) => absolutize(path),
    }
}

fn absolutize(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("resolve path '{}': {error}", path.display()))
    }
}

fn require_file(path: &Path, label: &str) -> Result<(), String> {
    path.is_file()
        .then_some(())
        .ok_or_else(|| format!("{label} is missing: '{}'", path.display()))
}

fn require_dir(path: &Path, label: &str) -> Result<(), String> {
    path.is_dir()
        .then_some(())
        .ok_or_else(|| format!("{label} is missing: '{}'", path.display()))
}

fn progress(value: f32, stage: &str) {
    println!(
        "NORTHSTAR_BUILD_PROGRESS|{:.3}|{}",
        value.clamp(0.0, 1.0),
        stage
    );
    let _ = io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_os_parser_matches_project_browser_ids() {
        assert_eq!(
            StandaloneTargetOs::from_id("windows"),
            Some(StandaloneTargetOs::Windows)
        );
        assert_eq!(
            StandaloneTargetOs::from_id("linux"),
            Some(StandaloneTargetOs::Linux)
        );
        assert_eq!(
            StandaloneTargetOs::from_id("macos"),
            Some(StandaloneTargetOs::MacOs)
        );
        assert_eq!(StandaloneTargetOs::from_id("beos"), None);
    }

    #[test]
    fn package_name_rejects_path_injection() {
        assert!(sanitize_package_name("Seattle").is_ok());
        assert!(sanitize_package_name("../Seattle").is_err());
        assert!(sanitize_package_name(r"C:\Seattle").is_err());
    }
}
