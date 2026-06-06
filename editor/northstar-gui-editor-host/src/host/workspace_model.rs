use northstar_gui_editor_assets::format_types::FormatTypeRegistry;
use crate::host::gateway_registry::GatewayRouteRegistry;
use northstar_gui_editor_gateway::registry::CapabilityRegistry;
use northstar_gui_editor_assets::workspace::{AssetWorkspace, AssetWorkspaceDto};

#[derive(Debug, Clone)]
pub struct WorkspacePanelDto {
    pub id: String,
    pub title: String,
    pub row_count: usize,
}

#[derive(Debug, Clone)]
pub struct EditorWorkspaceModel {
    pub assets: AssetWorkspaceDto,
    pub panels: Vec<WorkspacePanelDto>,
}

impl EditorWorkspaceModel {
    pub fn compose(
        workspace: &AssetWorkspace,
        registry: &CapabilityRegistry,
        type_registry: &FormatTypeRegistry,
        gateway_registry: &GatewayRouteRegistry,
    ) -> Self {
        Self {
            assets: workspace.describe(),
            panels: vec![
                WorkspacePanelDto { id: "asset_tree".to_owned(), title: "Asset Tree".to_owned(), row_count: workspace.describe().mounted_sources.len() },
                WorkspacePanelDto { id: "provider_tree".to_owned(), title: "Provider Tree".to_owned(), row_count: registry.providers().len() },
                WorkspacePanelDto { id: "capability_tree".to_owned(), title: "Capability Tree".to_owned(), row_count: registry.capability_count() },
                WorkspacePanelDto { id: "gateway_routes".to_owned(), title: "Gateway Routes".to_owned(), row_count: gateway_registry.routes().len() },
                WorkspacePanelDto { id: "format_type_browser".to_owned(), title: "Format Type Browser".to_owned(), row_count: type_registry.descriptors().len() },
                WorkspacePanelDto { id: "preview_surface".to_owned(), title: "Preview Surface".to_owned(), row_count: 0 },
                WorkspacePanelDto { id: "inspector_surface".to_owned(), title: "Inspector Surface".to_owned(), row_count: 0 },
                WorkspacePanelDto { id: "diagnostics_pane".to_owned(), title: "Diagnostics Pane".to_owned(), row_count: 0 },
            ],
        }
    }
}
