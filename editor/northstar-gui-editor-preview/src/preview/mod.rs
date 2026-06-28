#![allow(dead_code)]

use std::fs;

use northstar_gui_editor_assets::format_types::FormatTypeDescriptor;
use northstar_gui_editor_gateway::registry::ProviderDescriptor;
use northstar_gui_editor_assets::workspace::AssetRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewModel {
    pub asset_label: String,
    pub provider_id: String,
    pub surface: PreviewSurfaceDto,
    pub viewport: ViewportDto,
    pub asset_info: AssetInfoDto,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSurfaceDto {
    pub id: String,
    pub title: String,
    pub kind: PreviewSurfaceKind,
    pub accepts_binary_blob: bool,
    pub accepts_document_tree: bool,
    pub accepts_render_packets: bool,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSurfaceKind {
    Text,
    XmlHighlight,
    Image,
    Texture,
    Model,
    Material,
    Tree,
    BinaryHex,
    UnknownFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportDto {
    pub id: String,
    pub title: String,
    pub kind: ViewportKind,
    pub camera: Option<ViewportCameraDto>,
    pub overlays: Vec<ViewportOverlayDto>,
    pub toolbar: Vec<ViewportActionDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportKind {
    TextEditor,
    XmlSyntaxHighlighter,
    ImageViewer,
    TextureViewer,
    ModelViewer3d,
    MaterialPreview,
    TreeViewer,
    HexViewer,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportCameraDto {
    pub mode: String,
    pub focus: String,
    pub near_clip: String,
    pub far_clip: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportOverlayDto {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportActionDto {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfoDto {
    pub logical_path: String,
    pub absolute_path: String,
    pub extension: String,
    pub provider_id: String,
    pub provider_kind: String,
    pub file_size_bytes: Option<u64>,
    pub content_kind_hint: String,
    pub parameters: Vec<AssetParameterDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetParameterDto {
    pub key: String,
    pub label: String,
    pub value: String,
    pub status: AssetParameterStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetParameterStatus {
    Known,
    PendingProviderDecode,
    NotApplicable,
}

impl PreviewModel {
    pub fn for_provider(provider: &ProviderDescriptor, asset: &AssetRef) -> Self {
        Self::from_route(provider, None, asset)
    }

    pub fn from_route(provider: &ProviderDescriptor, format_type: Option<&FormatTypeDescriptor>, asset: &AssetRef) -> Self {
        let surface = PreviewSurfaceDto::from_route(provider, format_type);
        let viewport = ViewportDto::from_surface(&surface);
        let asset_info = AssetInfoDto::from_provider_and_asset(provider, asset, &surface, format_type);
        let mut diagnostics = Vec::new();

        if surface.kind == PreviewSurfaceKind::UnknownFallback {
            diagnostics.push(format!(
                "provider '{}' has no declared preview capability; using unknown fallback preview",
                provider.id
            ));
        }

        if format_type.is_none() {
            diagnostics.push("no matching format type descriptor was found for this asset route".to_owned());
        }

        if !asset.absolute_path.exists() {
            diagnostics.push(format!(
                "asset path is not present on disk yet: {}",
                asset.absolute_path.display()
            ));
        }

        Self {
            asset_label: asset.logical_path.display().to_string(),
            provider_id: provider.id.clone(),
            surface,
            viewport,
            asset_info,
            diagnostics,
        }
    }
}

impl PreviewSurfaceDto {
    pub fn from_route(provider: &ProviderDescriptor, format_type: Option<&FormatTypeDescriptor>) -> Self {
        if let Some(format_type) = format_type {
            if let Some(surface) = format_type.preview_surface.as_deref() {
                return Self::from_surface_id(surface);
            }
        }

        for capability in &provider.capabilities {
            if let Some(surface) = capability.strip_prefix("asset.preview.") {
                return Self::from_surface_id(surface);
            }
        }

        if provider.capabilities.iter().any(|capability| capability.ends_with(".read") || capability.ends_with(".validate")) {
            return Self::typed("preview.binary_hex", "Binary/Hex Preview", PreviewSurfaceKind::BinaryHex, true, false, false);
        }

        Self {
            id: "preview.unknown_fallback".to_owned(),
            title: "Unknown Fallback Preview".to_owned(),
            kind: PreviewSurfaceKind::UnknownFallback,
            accepts_binary_blob: true,
            accepts_document_tree: false,
            accepts_render_packets: false,
            fallback_reason: Some("no provider preview/read/inspect capability was declared".to_owned()),
        }
    }

    fn from_surface_id(surface: &str) -> Self {
        match normalize_surface_id(surface).as_str() {
            "text" => Self::typed("preview.text", "Text Preview", PreviewSurfaceKind::Text, true, true, false),
            "xml_highlight" | "xml" => Self::typed("preview.xml_highlight", "XML Highlight Preview", PreviewSurfaceKind::XmlHighlight, true, true, false),
            "image" => Self::typed("preview.image", "Image Preview", PreviewSurfaceKind::Image, true, false, true),
            "texture" | "texture_dictionary" => Self::typed("preview.texture", "Texture Preview", PreviewSurfaceKind::Texture, true, true, true),
            "model" | "drawable_dictionary" => Self::typed("preview.model", "Model Preview", PreviewSurfaceKind::Model, false, true, true),
            "material" => Self::typed("preview.material", "Material Preview", PreviewSurfaceKind::Material, false, true, true),
            "tree" | "list" => Self::typed("preview.tree", "Tree/List Preview", PreviewSurfaceKind::Tree, false, true, false),
            "binary_hex" | "hex" | "binary" => Self::typed("preview.binary_hex", "Binary/Hex Preview", PreviewSurfaceKind::BinaryHex, true, false, false),
            other => Self {
                id: format!("preview.{other}"),
                title: format!("{} Preview", title_case_token(other)),
                kind: PreviewSurfaceKind::UnknownFallback,
                accepts_binary_blob: true,
                accepts_document_tree: false,
                accepts_render_packets: false,
                fallback_reason: Some(format!("unknown descriptor preview_surface '{other}'")),
            },
        }
    }

    fn typed(id: &str, title: &str, kind: PreviewSurfaceKind, accepts_binary_blob: bool, accepts_document_tree: bool, accepts_render_packets: bool) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            kind,
            accepts_binary_blob,
            accepts_document_tree,
            accepts_render_packets,
            fallback_reason: None,
        }
    }
}

impl ViewportDto {
    pub fn from_surface(surface: &PreviewSurfaceDto) -> Self {
        match surface.kind {
            PreviewSurfaceKind::Text => Self::flat("viewport.text", "Text Viewport", ViewportKind::TextEditor),
            PreviewSurfaceKind::XmlHighlight => Self::flat("viewport.xml_highlight", "XML Highlight Viewport", ViewportKind::XmlSyntaxHighlighter),
            PreviewSurfaceKind::Image => Self::image("viewport.image", "Image Viewport", ViewportKind::ImageViewer),
            PreviewSurfaceKind::Texture => Self::image("viewport.texture", "Texture Viewport", ViewportKind::TextureViewer),
            PreviewSurfaceKind::Model => Self::model("viewport.model", "3D Model Viewport", ViewportKind::ModelViewer3d),
            PreviewSurfaceKind::Material => Self::model("viewport.material", "Material Preview Viewport", ViewportKind::MaterialPreview),
            PreviewSurfaceKind::Tree => Self::flat("viewport.tree", "Tree/List Viewport", ViewportKind::TreeViewer),
            PreviewSurfaceKind::BinaryHex => Self::flat("viewport.hex", "Binary/Hex Viewport", ViewportKind::HexViewer),
            PreviewSurfaceKind::UnknownFallback => Self::flat("viewport.fallback", "Fallback Viewport", ViewportKind::Fallback),
        }
    }

    fn flat(id: &str, title: &str, kind: ViewportKind) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            kind,
            camera: None,
            overlays: vec![ViewportOverlayDto::new("overlay.asset_info", "Asset info", true)],
            toolbar: vec![
                ViewportActionDto::new("viewport.copy_path", "Copy path", true),
                ViewportActionDto::new("viewport.open_external", "Open external", true),
            ],
        }
    }

    fn image(id: &str, title: &str, kind: ViewportKind) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            kind,
            camera: None,
            overlays: vec![
                ViewportOverlayDto::new("overlay.asset_info", "Asset info", true),
                ViewportOverlayDto::new("overlay.pixel_grid", "Pixel grid", false),
                ViewportOverlayDto::new("overlay.alpha_checker", "Alpha checker", true),
            ],
            toolbar: vec![
                ViewportActionDto::new("viewport.zoom_fit", "Zoom fit", true),
                ViewportActionDto::new("viewport.zoom_100", "100%", true),
                ViewportActionDto::new("viewport.channel_rgba", "RGBA", true),
            ],
        }
    }

    fn model(id: &str, title: &str, kind: ViewportKind) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            kind,
            camera: Some(ViewportCameraDto {
                mode: "orbit".to_owned(),
                focus: "asset_bounds".to_owned(),
                near_clip: "auto".to_owned(),
                far_clip: "auto".to_owned(),
            }),
            overlays: vec![
                ViewportOverlayDto::new("overlay.asset_info", "Asset info", true),
                ViewportOverlayDto::new("overlay.grid", "Grid", true),
                ViewportOverlayDto::new("overlay.bounds", "Bounds", true),
                ViewportOverlayDto::new("overlay.wireframe", "Wireframe", false),
            ],
            toolbar: vec![
                ViewportActionDto::new("viewport.frame_asset", "Frame asset", true),
                ViewportActionDto::new("viewport.toggle_grid", "Grid", true),
                ViewportActionDto::new("viewport.toggle_wireframe", "Wireframe", true),
            ],
        }
    }
}

impl ViewportOverlayDto {
    fn new(id: &str, label: &str, enabled: bool) -> Self {
        Self { id: id.to_owned(), label: label.to_owned(), enabled }
    }
}

impl ViewportActionDto {
    fn new(id: &str, label: &str, enabled: bool) -> Self {
        Self { id: id.to_owned(), label: label.to_owned(), enabled }
    }
}

impl AssetInfoDto {
    pub fn from_provider_and_asset(provider: &ProviderDescriptor, asset: &AssetRef, surface: &PreviewSurfaceDto, format_type: Option<&FormatTypeDescriptor>) -> Self {
        let file_size_bytes = fs::metadata(&asset.absolute_path).ok().map(|metadata| metadata.len());
        let extension = asset.extension.clone().unwrap_or_else(|| "<none>".to_owned());
        let content_kind_hint = content_kind_hint(surface, format_type);
        let mut parameters = vec![
            AssetParameterDto::known("path.logical", "Logical path", asset.logical_path.display().to_string()),
            AssetParameterDto::known("provider.id", "Provider", provider.id.clone()),
            AssetParameterDto::known("provider.kind", "Provider kind", provider.kind.clone()),
            AssetParameterDto::known("preview.surface", "Preview surface", surface.title.clone()),
        ];

        match file_size_bytes {
            Some(size) => parameters.push(AssetParameterDto::known("file.size", "File size", format!("{} bytes", size))),
            None => parameters.push(AssetParameterDto::pending("file.size", "File size", "missing or not readable")),
        }

        match surface.kind {
            PreviewSurfaceKind::Image | PreviewSurfaceKind::Texture => {
                parameters.push(AssetParameterDto::pending("image.width", "Width", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("image.height", "Height", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("image.format", "Pixel format", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("image.mip_count", "Mip count", "pending provider decode"));
            }
            PreviewSurfaceKind::Model => {
                parameters.push(AssetParameterDto::pending("model.vertex_count", "Vertex count", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("model.index_count", "Index count", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("model.mesh_count", "Mesh count", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("model.material_count", "Material count", "pending provider decode"));
            }
            PreviewSurfaceKind::Material => {
                parameters.push(AssetParameterDto::pending("material.shader", "Shader", "pending provider decode"));
                parameters.push(AssetParameterDto::pending("material.texture_slots", "Texture slots", "pending provider decode"));
            }
            PreviewSurfaceKind::Text | PreviewSurfaceKind::XmlHighlight => {
                parameters.push(AssetParameterDto::pending("text.encoding", "Encoding", "pending text decode"));
                parameters.push(AssetParameterDto::pending("text.line_count", "Line count", "pending text decode"));
            }
            PreviewSurfaceKind::Tree | PreviewSurfaceKind::BinaryHex | PreviewSurfaceKind::UnknownFallback => {}
        }

        Self {
            logical_path: asset.logical_path.display().to_string(),
            absolute_path: asset.absolute_path.display().to_string(),
            extension,
            provider_id: provider.id.clone(),
            provider_kind: provider.kind.clone(),
            file_size_bytes,
            content_kind_hint,
            parameters,
        }
    }
}

impl AssetParameterDto {
    fn known(key: &str, label: &str, value: String) -> Self {
        Self { key: key.to_owned(), label: label.to_owned(), value, status: AssetParameterStatus::Known }
    }

    fn pending(key: &str, label: &str, value: &str) -> Self {
        Self { key: key.to_owned(), label: label.to_owned(), value: value.to_owned(), status: AssetParameterStatus::PendingProviderDecode }
    }
}

fn content_kind_hint(surface: &PreviewSurfaceDto, format_type: Option<&FormatTypeDescriptor>) -> String {
    if let Some(format_type) = format_type {
        return format_type.content_kind.clone();
    }

    match surface.kind {
        PreviewSurfaceKind::Text => "text_document".to_owned(),
        PreviewSurfaceKind::XmlHighlight => "xml_document".to_owned(),
        PreviewSurfaceKind::Image => "image_asset".to_owned(),
        PreviewSurfaceKind::Texture => "texture_asset".to_owned(),
        PreviewSurfaceKind::Model => "model_asset".to_owned(),
        PreviewSurfaceKind::Material => "material_asset".to_owned(),
        PreviewSurfaceKind::Tree => "tree_asset".to_owned(),
        PreviewSurfaceKind::BinaryHex => "binary_asset".to_owned(),
        PreviewSurfaceKind::UnknownFallback => "asset".to_owned(),
    }
}

fn normalize_surface_id(surface: &str) -> String {
    surface.trim().trim_start_matches("preview.").to_ascii_lowercase().replace('-', "_")
}

fn title_case_token(token: &str) -> String {
    token
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
