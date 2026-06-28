use crate::inspector::InspectorModel;
use crate::preview::PreviewModel;
use crate::registry::ProviderDescriptor;
use crate::workspace::AssetRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorShellModel {
    pub title: String,
    pub selected_provider_id: String,
    pub layout: EditorLayoutDto,
    pub menus: Vec<MenuDto>,
    pub panels: Vec<PanelDto>,
    pub preview: PreviewModel,
    pub inspector: InspectorModel,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorLayoutDto {
    pub left_panel: String,
    pub center_surface: String,
    pub right_panel: String,
    pub bottom_panel: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuDto {
    pub id: String,
    pub label: String,
    pub actions: Vec<ActionDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDto {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub required_capability: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelDto {
    pub id: String,
    pub title: String,
    pub kind: PanelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    ProjectTree,
    PreviewSurface,
    Inspector,
    Diagnostics,
    ToolExecutionLog,
}

impl EditorShellModel {
    pub fn compose(
        provider: &ProviderDescriptor,
        asset: &AssetRef,
        preview: PreviewModel,
        inspector: InspectorModel,
    ) -> Self {
        let mut diagnostics = Vec::new();
        diagnostics.extend(preview.diagnostics.clone());
        diagnostics.extend(inspector.validation.messages.clone());

        Self {
            title: format!("NorthStar GUI Editor — {}", asset.logical_path.display()),
            selected_provider_id: provider.id.clone(),
            layout: EditorLayoutDto {
                left_panel: "asset_workspace.project_tree".to_owned(),
                center_surface: preview.surface.id.clone(),
                right_panel: inspector.schema.id.clone(),
                bottom_panel: "diagnostics.log_stream".to_owned(),
            },
            menus: build_menus(provider),
            panels: vec![
                PanelDto {
                    id: "asset_workspace.project_tree".to_owned(),
                    title: "Asset Workspace".to_owned(),
                    kind: PanelKind::ProjectTree,
                },
                PanelDto {
                    id: preview.surface.id.clone(),
                    title: preview.surface.title.clone(),
                    kind: PanelKind::PreviewSurface,
                },
                PanelDto {
                    id: inspector.schema.id.clone(),
                    title: inspector.schema.title.clone(),
                    kind: PanelKind::Inspector,
                },
                PanelDto {
                    id: "diagnostics.log_stream".to_owned(),
                    title: "Diagnostics".to_owned(),
                    kind: PanelKind::Diagnostics,
                },
                PanelDto {
                    id: "tools.execution_log".to_owned(),
                    title: "Tool Execution".to_owned(),
                    kind: PanelKind::ToolExecutionLog,
                },
            ],
            preview,
            inspector,
            diagnostics,
        }
    }

    pub fn print_summary(&self) {
        println!("[SHELL] title={}", self.title);
        println!("[SHELL] provider={}", self.selected_provider_id);
        println!("[SHELL] left_panel={}", self.layout.left_panel);
        println!("[SHELL] center_surface={}", self.layout.center_surface);
        println!("[SHELL] right_panel={}", self.layout.right_panel);
        println!("[SHELL] bottom_panel={}", self.layout.bottom_panel);
        println!(
            "[PREVIEW] kind={:?} title={}",
            self.preview.surface.kind, self.preview.surface.title
        );
        println!(
            "[VIEWPORT] id={} title={} kind={:?}",
            self.preview.viewport.id, self.preview.viewport.title, self.preview.viewport.kind
        );
        println!(
            "[ASSET-INFO] provider={} size={:?} content_kind={}",
            self.preview.asset_info.provider_id,
            self.preview.asset_info.file_size_bytes,
            self.preview.asset_info.content_kind_hint
        );
        for parameter in &self.preview.asset_info.parameters {
            println!(
                "  [PARAM] {}={} status={:?}",
                parameter.label, parameter.value, parameter.status
            );
        }
        println!(
            "[INSPECTOR] schema={} sections={}",
            self.inspector.schema.id,
            self.inspector.schema.sections.len()
        );
        println!(
            "[TRANSACTION] dirty={} write_back_allowed={}",
            self.inspector.transaction.dirty, self.inspector.transaction.write_back_allowed
        );
        for diagnostic in &self.diagnostics {
            println!("[DIAG] {diagnostic}");
        }
    }
}

fn build_menus(provider: &ProviderDescriptor) -> Vec<MenuDto> {
    vec![
        MenuDto {
            id: "file".to_owned(),
            label: "File".to_owned(),
            actions: vec![
                action("file.open", "Open", true, None),
                action(
                    "file.save",
                    "Save",
                    has_capability(provider, "write"),
                    Some("asset.format.write"),
                ),
                action(
                    "file.validate",
                    "Validate",
                    has_capability(provider, "validate"),
                    Some("asset.format.validate"),
                ),
            ],
        },
        MenuDto {
            id: "view".to_owned(),
            label: "View".to_owned(),
            actions: vec![
                action("view.preview", "Preview", true, None),
                action(
                    "view.inspect",
                    "Inspector",
                    has_capability(provider, "inspect"),
                    Some("asset.format.inspect"),
                ),
                action("view.hex", "Binary/Hex", true, None),
            ],
        },
        MenuDto {
            id: "tools".to_owned(),
            label: "Tools".to_owned(),
            actions: vec![
                action(
                    "tools.diff",
                    "Diff",
                    has_capability(provider, "diff"),
                    Some("asset.format.diff"),
                ),
                action(
                    "tools.edit_schema",
                    "Edit Schema",
                    has_capability(provider, "edit_schema"),
                    Some("asset.editor.*.edit_schema"),
                ),
            ],
        },
    ]
}

fn action(id: &str, label: &str, enabled: bool, required_capability: Option<&str>) -> ActionDto {
    ActionDto {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled,
        required_capability: required_capability.map(ToOwned::to_owned),
    }
}

fn has_capability(provider: &ProviderDescriptor, fragment: &str) -> bool {
    provider
        .capabilities
        .iter()
        .any(|capability| capability.contains(fragment))
}
