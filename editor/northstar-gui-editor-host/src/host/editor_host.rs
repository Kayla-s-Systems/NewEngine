use std::path::{Path, PathBuf};

use crate::discovery::Discovery;
use northstar_gui_editor_assets::format_types::{FormatTypeRegistry, RuntimeTypeRegistration};
use crate::host::diagnostics::EditorDiagnostic;
use crate::host::gateway_registry::GatewayRouteRegistry;
use crate::host::paths::EditorPaths;
use crate::host::workspace_model::EditorWorkspaceModel;
use northstar_gui_editor_gateway::registry::{CapabilityRegistry, GatewayRegistry, ProviderDescriptor};
use crate::tool_runtime::discover_remembered_self_describing_tools;
use northstar_gui_editor_assets::workspace::AssetWorkspace;

#[derive(Debug, Clone)]
pub struct EditorHost {
    pub paths: EditorPaths,
    pub providers: Vec<ProviderDescriptor>,
    pub capability_registry: CapabilityRegistry,
    pub gateway_registry: GatewayRouteRegistry,
    pub routes: GatewayRegistry,
    pub type_registry: FormatTypeRegistry,
    pub workspace: AssetWorkspace,
    pub workspace_model: EditorWorkspaceModel,
    pub diagnostics: Vec<EditorDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct EditorHostSnapshot {
    pub root: PathBuf,
    pub providers: usize,
    pub capabilities: usize,
    pub gateway_routes: usize,
    pub format_types: usize,
    pub diagnostics: usize,
}

impl EditorHost {
    pub fn load(root: PathBuf) -> Result<Self, String> {
        let paths = EditorPaths::new(root);
        let mut diagnostics = Vec::new();
        diagnostics.push(EditorDiagnostic::info("host", "loading descriptor-driven editor host"));

        let discovery = Discovery::new(paths.newengine_root.clone());
        let mut providers = discovery.discover()?;
        let mut type_registry = FormatTypeRegistry::load_from_roots(&paths.format_type_roots())?;

        let remembered_tools = discover_remembered_self_describing_tools(&paths.newengine_root)?;
        for diagnostic in &remembered_tools.diagnostics {
            diagnostics.push(EditorDiagnostic::info("tool_runtime", diagnostic.clone()));
        }
        for registration in remembered_tools.registrations {
            type_registry.register_runtime(registration)?;
        }
        providers.extend(remembered_tools.providers);

        let capability_registry = CapabilityRegistry::from_providers(providers.clone());
        let gateway_registry = GatewayRouteRegistry::from_providers(&providers);
        let routes = GatewayRegistry::from_format_types(&capability_registry, type_registry.descriptors());
        let workspace = AssetWorkspace::new(paths.newengine_root.clone());
        let workspace_model = EditorWorkspaceModel::compose(&workspace, &capability_registry, &type_registry, &gateway_registry);

        if providers.is_empty() {
            diagnostics.push(EditorDiagnostic::error("discovery", "no tool/codec/editor providers discovered"));
        }
        if type_registry.descriptors().is_empty() {
            diagnostics.push(EditorDiagnostic::error("format_types", "no format type descriptors discovered"));
        }
        if gateway_registry.routes().is_empty() {
            diagnostics.push(EditorDiagnostic::warn("gateway", "no gateway routes were projected from providers"));
        }

        Ok(Self {
            paths,
            providers,
            capability_registry,
            gateway_registry,
            routes,
            type_registry,
            workspace,
            workspace_model,
            diagnostics,
        })
    }

    pub fn snapshot(&self) -> EditorHostSnapshot {
        EditorHostSnapshot {
            root: self.paths.newengine_root.clone(),
            providers: self.providers.len(),
            capabilities: self.capability_registry.capability_count(),
            gateway_routes: self.routes.routes().len(),
            format_types: self.type_registry.descriptors().len(),
            diagnostics: self.diagnostics.len(),
        }
    }

    pub fn register_runtime_type(&mut self, registration: RuntimeTypeRegistration) -> Result<(), String> {
        self.type_registry.register_runtime(registration)?;
        self.refresh_workspace_model();
        Ok(())
    }

    pub fn load_type_directory(&mut self, dir: &Path) -> Result<(), String> {
        self.type_registry.load_directory(dir)?;
        self.refresh_workspace_model();
        Ok(())
    }

    fn refresh_workspace_model(&mut self) {
        self.workspace_model = EditorWorkspaceModel::compose(
            &self.workspace,
            &self.capability_registry,
            &self.type_registry,
            &self.gateway_registry,
        );
    }
}

impl EditorHostSnapshot {
    pub fn print(&self) {
        println!("[HOST] root={}", self.root.display());
        println!("[HOST] providers={}", self.providers);
        println!("[HOST] capabilities={}", self.capabilities);
        println!("[HOST] gateway_routes={}", self.gateway_routes);
        println!("[HOST] format_types={}", self.format_types);
        println!("[HOST] diagnostics={}", self.diagnostics);
    }
}
