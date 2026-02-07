use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StartupPaths {
    root: PathBuf,
}

impl StartupPaths {
    #[inline]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[inline]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("startup.json")
    }
}