use crate::types::AssetError;
use std::path::{Path, PathBuf};

pub trait AssetSource: Send + Sync + 'static {
    fn exists(&self, logical_path: &Path) -> bool;
    fn read(&self, logical_path: &Path) -> Result<Vec<u8>, AssetError>;
}

#[derive(Debug, Clone)]
pub struct FileSystemSource {
    root: PathBuf,
}

impl FileSystemSource {
    #[inline]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[inline]
    fn resolve(&self, logical_path: &Path) -> PathBuf {
        let mut p = self.root.clone();
        p.push(logical_path);
        p
    }
}

impl AssetSource for FileSystemSource {
    #[inline]
    fn exists(&self, logical_path: &Path) -> bool {
        self.resolve(logical_path).exists()
    }

    fn read(&self, logical_path: &Path) -> Result<Vec<u8>, AssetError> {
        let p = self.resolve(logical_path);
        std::fs::read(&p).map_err(|e| {
            AssetError::new(format!(
                "FileSystemSource: failed to read '{}': {}",
                p.to_string_lossy(),
                e
            ))
        })
    }
}