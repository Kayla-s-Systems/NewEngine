#![allow(dead_code)]

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWorkspace {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef {
    pub workspace_root: PathBuf,
    pub logical_path: PathBuf,
    pub absolute_path: PathBuf,
    pub extension: Option<String>,
    pub source_kind: AssetSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSourceKind {
    LooseFile,
    MountedArchiveEntry,
    VirtualFileSystemEntry,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWorkspaceDto {
    pub root: PathBuf,
    pub mounted_sources: Vec<MountedSourceDto>,
    pub graph_nodes: Vec<DecodedAssetGraphNodeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedSourceDto {
    pub id: String,
    pub label: String,
    pub kind: AssetSourceKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAssetGraphNodeDto {
    pub id: String,
    pub label: String,
    pub logical_path: PathBuf,
    pub provider_id: Option<String>,
    pub capabilities: Vec<String>,
}

impl AssetWorkspace {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn resolve(&self, logical_path: PathBuf) -> AssetRef {
        let normalized = normalize_logical_path(logical_path);
        let absolute_path = if normalized.is_absolute() || normalized.exists() {
            normalized.clone()
        } else if normalized.starts_with(&self.root) {
            normalized.clone()
        } else {
            self.root.join(&normalized)
        };

        let extension = normalized
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}").to_ascii_lowercase());

        AssetRef {
            workspace_root: self.root.clone(),
            logical_path: normalized,
            absolute_path,
            extension,
            source_kind: AssetSourceKind::LooseFile,
        }
    }

    pub fn describe(&self) -> AssetWorkspaceDto {
        AssetWorkspaceDto {
            root: self.root.clone(),
            mounted_sources: vec![MountedSourceDto {
                id: "workspace.loose_files".to_owned(),
                label: "Loose files".to_owned(),
                kind: AssetSourceKind::LooseFile,
                path: self.root.clone(),
            }],
            graph_nodes: Vec::new(),
        }
    }
}

fn normalize_logical_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
