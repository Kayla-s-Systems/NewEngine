use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPaths {
    pub newengine_root: PathBuf,
    pub editor_root: PathBuf,
    pub editor_tool_root: PathBuf,
    pub format_type_root: PathBuf,
}

impl EditorPaths {
    pub fn new(newengine_root: PathBuf) -> Self {
        let editor_root = newengine_root.join("editor").join("northstar-gui-editor");
        let editor_tool_root = editor_root.join("tools").join("first_party");
        let format_type_root = editor_root.join("format_types");
        Self { newengine_root, editor_root, editor_tool_root, format_type_root }
    }

    pub fn format_type_roots(&self) -> [PathBuf; 2] {
        [self.editor_tool_root.clone(), self.format_type_root.clone()]
    }

    pub fn looks_like_newengine_root(path: &Path) -> bool {
        path.join("editor").join("northstar-gui-editor").exists()
            || path.join("neocore2").exists()
    }
}
