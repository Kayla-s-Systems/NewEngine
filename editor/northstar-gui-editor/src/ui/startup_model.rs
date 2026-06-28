use std::path::PathBuf;

use crate::format_types::FormatTypeRegistry;
use crate::registry::CapabilityRegistry;

use super::{PanelDto, PanelKind};

pub struct EditorStartupModel {
    pub title: String,
    pub root: PathBuf,
    pub provider_count: usize,
    pub capability_count: usize,
    pub format_type_count: usize,
    pub preview_provider_count: usize,
    pub panels: Vec<PanelDto>,
    pub provider_ids: Vec<String>,
    pub format_type_ids: Vec<String>,
    pub tool_routes: Vec<crate::tool_runtime::ToolRouteDescriptor>,
}

impl EditorStartupModel {
    pub fn from_registries(
        root: PathBuf,
        registry: &CapabilityRegistry,
        type_registry: &FormatTypeRegistry,
    ) -> Self {
        let mut provider_ids: Vec<String> = registry
            .providers()
            .iter()
            .map(|provider| provider.id.clone())
            .collect();
        provider_ids.sort();

        let mut format_type_ids: Vec<String> = type_registry
            .descriptors()
            .iter()
            .map(|descriptor| descriptor.type_id.clone())
            .collect();
        format_type_ids.sort();
        let tool_routes = crate::tool_runtime::routes_from_providers(registry.providers());

        Self {
            title: "NorthStar GUI Editor".to_owned(),
            root,
            provider_count: registry.providers().len(),
            capability_count: registry.capability_count(),
            format_type_count: type_registry.descriptors().len(),
            preview_provider_count: registry
                .providers_with_capability_fragment("preview.")
                .len(),
            panels: vec![
                PanelDto {
                    id: "asset_workspace.project_tree".to_owned(),
                    title: "Asset Workspace".to_owned(),
                    kind: PanelKind::ProjectTree,
                },
                PanelDto {
                    id: "preview.viewport".to_owned(),
                    title: "Preview Viewport".to_owned(),
                    kind: PanelKind::PreviewSurface,
                },
                PanelDto {
                    id: "inspector.properties".to_owned(),
                    title: "Inspector".to_owned(),
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
            provider_ids,
            format_type_ids,
            tool_routes,
        }
    }

    pub fn print_summary(&self) {
        println!("[UI] launch title={}", self.title);
        println!("[UI] root={}", self.root.display());
        println!("[UI] providers={}", self.provider_count);
        println!("[UI] capabilities={}", self.capability_count);
        println!("[UI] format_types={}", self.format_type_count);
        println!("[UI] preview_providers={}", self.preview_provider_count);
        println!("[UI] tool_routes={}", self.tool_routes.len());
        for panel in &self.panels {
            println!(
                "[UI][PANEL] id={} title={} kind={:?}",
                panel.id, panel.title, panel.kind
            );
        }
    }

    pub fn render_text(&self) -> String {
        let mut text = String::new();
        text.push_str("NorthStar GUI Editor\n");
        text.push_str("====================\n\n");
        text.push_str(&format!("Root: {}\n", self.root.display()));
        text.push_str(&format!("Providers: {}\n", self.provider_count));
        text.push_str(&format!("Capabilities: {}\n", self.capability_count));
        text.push_str(&format!("Format types: {}\n", self.format_type_count));
        text.push_str(&format!(
            "Preview providers: {}\n\n",
            self.preview_provider_count
        ));
        text.push_str("Panels:\n");
        for panel in &self.panels {
            text.push_str(&format!("  - {} ({:?})\n", panel.title, panel.kind));
        }
        text.push_str("\nProviders:\n");
        for provider in self.provider_ids.iter().take(16) {
            text.push_str(&format!("  - {provider}\n"));
        }
        if self.provider_ids.len() > 16 {
            text.push_str(&format!("  ... {} more\n", self.provider_ids.len() - 16));
        }
        text.push_str("\nFormat types:\n");
        for type_id in self.format_type_ids.iter().take(16) {
            text.push_str(&format!("  - {type_id}\n"));
        }
        if self.format_type_ids.len() > 16 {
            text.push_str(&format!("  ... {} more\n", self.format_type_ids.len() - 16));
        }
        text
    }
}
