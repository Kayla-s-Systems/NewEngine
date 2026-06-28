use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIconDescriptor {
    pub icon_id: String,
    pub content_kind: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct FileIconRegistry {
    icons_by_content_kind: BTreeMap<String, FileIconDescriptor>,
    default_icon: Option<FileIconDescriptor>,
    unknown_icon: Option<FileIconDescriptor>,
}

impl FileIconRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_icon_names(asset_root: impl AsRef<Path>) -> Self {
        let root = asset_root.as_ref();
        let mut registry = Self::new();
        registry.default_icon = Some(FileIconDescriptor {
            icon_id: "default".to_owned(),
            content_kind: "default".to_owned(),
            path: root.join("default.ico"),
        });
        registry.unknown_icon = Some(FileIconDescriptor {
            icon_id: "unknown".to_owned(),
            content_kind: "unknown".to_owned(),
            path: root.join("unknown.ico"),
        });
        registry.register("xml", "xml_document", root.join("xml.ico"));
        registry.register("text", "text_document", root.join("txt.ico"));
        registry.register("image_png", "image_png", root.join("png.ico"));
        registry.register("image_jpeg", "image_jpeg", root.join("jpeg.ico"));
        registry.register("audio_metadata", "audio_metadata", root.join("audio_metadata.ico"));
        registry.register("video", "video_asset", root.join("video.ico"));
        registry.register("resource", "resource_container", root.join("resource.ico"));
        registry.register("rpf", "rpf_archive", root.join("rpf.ico"));
        registry
    }

    pub fn register(&mut self, icon_id: impl Into<String>, content_kind: impl Into<String>, path: impl Into<PathBuf>) {
        let descriptor = FileIconDescriptor {
            icon_id: icon_id.into(),
            content_kind: content_kind.into(),
            path: path.into(),
        };
        self.icons_by_content_kind
            .insert(descriptor.content_kind.clone(), descriptor);
    }

    pub fn icon_for_content_kind(&self, content_kind: &str) -> Option<&FileIconDescriptor> {
        self.icons_by_content_kind
            .get(content_kind)
            .or(self.default_icon.as_ref())
            .or(self.unknown_icon.as_ref())
    }

    pub fn unknown_icon(&self) -> Option<&FileIconDescriptor> {
        self.unknown_icon.as_ref()
    }

    pub fn len(&self) -> usize {
        self.icons_by_content_kind.len()
    }

    pub fn is_empty(&self) -> bool {
        self.icons_by_content_kind.is_empty()
    }
}
