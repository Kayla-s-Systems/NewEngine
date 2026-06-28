use std::path::PathBuf;

use crate::format_types::{FormatTypeCapabilities, FormatTypeRegistry, RuntimeTypeRegistration};
use crate::host::EditorHost;
use crate::inspector::InspectorModel;
use crate::preview::PreviewModel;
use crate::tool_runtime::{discover_self_describing_tools, ToolMountStore};
use crate::tools::ToolPlaneBridge;
use crate::ui::{run_editor_ui, EditorShellModel, EditorStartupModel};

pub enum EditorCommand {
    LaunchUi,
    Doctor,
    DoctorWithTools,
    List,
    TypesList,
    TypesLoadDir(PathBuf),
    TypesAddRuntime {
        type_id: String,
        label: String,
        content_kind: String,
        extensions: Vec<String>,
        provider_id: Option<String>,
        can_read: bool,
        can_write: bool,
        can_preview: bool,
        can_edit_schema: bool,
        can_validate: bool,
        can_diff: bool,
    },
    ToolsList,
    ToolsDoctor,
    ToolsLoadDir(PathBuf),
    OpenAsset(PathBuf),
}

pub struct EditorApp {
    root: PathBuf,
}

impl EditorApp {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn run(&self, command: EditorCommand) -> Result<(), String> {
        match command {
            EditorCommand::ToolsList => self.tools_list(),
            EditorCommand::ToolsDoctor => self.tools_doctor(),
            EditorCommand::ToolsLoadDir(dir) => self.tools_load_dir(dir),
            other => {
                let mut host = EditorHost::load(self.root.clone())?;
                for diagnostic in &host.diagnostics {
                    diagnostic.print();
                }

                match other {
                    EditorCommand::LaunchUi => self.launch_ui(&host),
                    EditorCommand::Doctor => self.doctor(&host),
                    EditorCommand::DoctorWithTools => self.doctor_with_tools(&host),
                    EditorCommand::List => self.list(&host),
                    EditorCommand::TypesList => self.types_list(&host.type_registry),
                    EditorCommand::TypesLoadDir(dir) => {
                        host.load_type_directory(&dir)?;
                        println!("[TYPES] loaded_runtime_dir={}", dir.display());
                        self.types_list(&host.type_registry)
                    }
                    EditorCommand::TypesAddRuntime {
                        type_id,
                        label,
                        content_kind,
                        extensions,
                        provider_id,
                        can_read,
                        can_write,
                        can_preview,
                        can_edit_schema,
                        can_validate,
                        can_diff,
                    } => {
                        host.register_runtime_type(RuntimeTypeRegistration {
                            type_id,
                            label,
                            content_kind,
                            extensions,
                            provider_id,
                            capabilities: FormatTypeCapabilities {
                                can_read,
                                can_write,
                                can_inspect: can_read,
                                can_validate,
                                can_diff,
                                can_preview,
                                can_edit_schema,
                            },
                            schema_id: None,
                            preview_surface: None,
                            viewport: None,
                        })?;
                        self.types_list(&host.type_registry)
                    }
                    EditorCommand::OpenAsset(asset) => self.open_asset(&host, asset),
                    EditorCommand::ToolsList
                    | EditorCommand::ToolsDoctor
                    | EditorCommand::ToolsLoadDir(_) => {
                        unreachable!("tool commands handled before discovery")
                    }
                }
            }
        }
    }

    fn launch_ui(&self, host: &EditorHost) -> Result<(), String> {
        let startup = EditorStartupModel::from_registries(
            host.paths.newengine_root.clone(),
            &host.capability_registry,
            &host.type_registry,
        );
        startup.print_summary();
        run_editor_ui(&startup)
    }

    fn doctor(&self, host: &EditorHost) -> Result<(), String> {
        let snapshot = host.snapshot();
        snapshot.print();
        println!(
            "[INFO] NorthStar GUI Editor root={}",
            host.paths.newengine_root.display()
        );
        println!(
            "[INFO] providers={}",
            host.capability_registry.providers().len()
        );
        println!(
            "[INFO] capabilities={}",
            host.capability_registry.capability_count()
        );

        if host.capability_registry.providers().is_empty() {
            return Err("no tool/codec/editor providers discovered".into());
        }

        let read_count = host
            .capability_registry
            .providers_with_capability_suffix(".read")
            .len();
        let inspect_count = host
            .capability_registry
            .providers_with_capability_suffix(".inspect")
            .len();
        let validate_count = host
            .capability_registry
            .providers_with_capability_suffix(".validate")
            .len();
        let preview_count = host
            .capability_registry
            .providers_with_capability_fragment("preview.")
            .len();
        let type_summary = host.type_registry.capability_summary();

        println!("[CHECK] read providers={read_count}");
        println!("[CHECK] inspect providers={inspect_count}");
        println!("[CHECK] validate providers={validate_count}");
        println!("[CHECK] preview providers={preview_count}");
        println!("[CHECK] format types total={}", type_summary.total);
        println!("[CHECK] format types can_read={}", type_summary.can_read);
        println!("[CHECK] format types can_write={}", type_summary.can_write);
        println!(
            "[CHECK] format types can_preview={}",
            type_summary.can_preview
        );
        println!(
            "[CHECK] format types can_edit_schema={}",
            type_summary.can_edit_schema
        );
        println!(
            "[CHECK] format types can_validate={}",
            type_summary.can_validate
        );

        if read_count == 0 {
            println!("[WARN] no explicit *.read capability discovered yet");
        }
        if preview_count == 0 {
            println!("[WARN] no explicit asset.preview.* capability discovered yet; fallback preview will be used");
        }
        if type_summary.total == 0 {
            return Err(
                "no format type descriptors discovered; packers must publish format_type.json"
                    .into(),
            );
        }
        if type_summary.can_read == 0 {
            return Err("no format type declares can_read=true".into());
        }

        println!("[OK] descriptor-driven editor host discovery completed");
        Ok(())
    }

    fn doctor_with_tools(&self, host: &EditorHost) -> Result<(), String> {
        self.doctor(host)?;
        println!("[INFO] running external tool-plane diagnostics");

        let bridge = ToolPlaneBridge::new(self.root.clone());
        let list_result = bridge.tools_list();
        list_result.print();

        let doctor_result = bridge.tools_doctor();
        doctor_result.print();

        if !list_result.available || !doctor_result.available {
            println!("[WARN] tool-plane unavailable; local editor discovery remains usable");
            println!("[OK] editor discovery passed; external tool-plane capability is degraded");
            return Ok(());
        }

        if !list_result.is_success() || !doctor_result.is_success() {
            println!("[WARN] external tool-plane diagnostics failed; local editor discovery remains usable");
            println!("[OK] editor discovery passed; external tool-plane capability is degraded");
            return Ok(());
        }

        println!("[OK] editor discovery and external tool-plane diagnostics passed");
        Ok(())
    }

    fn tools_list(&self) -> Result<(), String> {
        let result = ToolPlaneBridge::new(self.root.clone()).tools_list();
        result.print();
        if result.available && !result.is_success() {
            Err("tools.list failed".to_owned())
        } else {
            Ok(())
        }
    }

    fn tools_doctor(&self) -> Result<(), String> {
        let result = ToolPlaneBridge::new(self.root.clone()).tools_doctor();
        result.print();
        if result.available && !result.is_success() {
            Err("tools.doctor failed".to_owned())
        } else {
            Ok(())
        }
    }

    fn tools_load_dir(&self, dir: PathBuf) -> Result<(), String> {
        let result = discover_self_describing_tools(&dir)?;
        for diagnostic in &result.diagnostics {
            println!("[TOOLS][LOAD-DIR] {diagnostic}");
        }

        ToolMountStore::remember_result(&self.root, &dir, &result)?;

        let mut host = EditorHost::load(self.root.clone())?;
        for registration in result.registrations.clone() {
            host.register_runtime_type(registration)?;
        }

        println!("[TOOLS] loaded_runtime_dir={}", dir.display());
        println!("[TOOLS] accepted_tools={}", result.providers.len());
        for provider in &result.providers {
            println!(
                "[TOOLS][ACCEPTED] id={} source={} formats={} capabilities={}",
                provider.id,
                provider.source.display(),
                provider.formats.join(","),
                provider.capabilities.join(",")
            );
        }
        self.types_list(&host.type_registry)
    }

    fn list(&self, host: &EditorHost) -> Result<(), String> {
        println!("[GATEWAYS]");
        for route in host.gateway_registry.routes() {
            println!(
                "[GATEWAY] id={} service_kind={} provider={} active={} reason={}",
                route.gateway_id,
                route.service_kind,
                route.provider_id,
                route.active,
                route.selection_reason
            );
        }
        println!("[WORKSPACE]");
        println!(
            "[WORKSPACE] mounted_sources={}",
            host.workspace_model.assets.mounted_sources.len()
        );
        for panel in &host.workspace_model.panels {
            println!(
                "[PANEL] id={} title={} rows={}",
                panel.id, panel.title, panel.row_count
            );
        }
        println!("[PROVIDERS]");
        for provider in host.capability_registry.providers() {
            println!(
                "[PROVIDER] id={} kind={} source={}",
                provider.id,
                provider.kind,
                provider.source.display()
            );
            for format in &provider.formats {
                println!("  [FORMAT] {format}");
            }
            for capability in &provider.capabilities {
                println!("  [CAPABILITY] {capability}");
            }
        }
        Ok(())
    }

    fn types_list(&self, type_registry: &FormatTypeRegistry) -> Result<(), String> {
        for descriptor in type_registry.descriptors() {
            println!(
                "[TYPE] id={} label={} provider={} content_kind={}",
                descriptor.type_id,
                descriptor.label,
                descriptor.provider_id.as_deref().unwrap_or("<none>"),
                descriptor.content_kind
            );
            println!("  [EXTENSIONS] {}", descriptor.extensions.join(","));
            println!(
                "  [CAN] read={} write={} inspect={} validate={} diff={} preview={} edit_schema={}",
                descriptor.capabilities.can_read,
                descriptor.capabilities.can_write,
                descriptor.capabilities.can_inspect,
                descriptor.capabilities.can_validate,
                descriptor.capabilities.can_diff,
                descriptor.capabilities.can_preview,
                descriptor.capabilities.can_edit_schema
            );
        }
        Ok(())
    }

    fn open_asset(&self, host: &EditorHost, asset: PathBuf) -> Result<(), String> {
        let asset_ref = host.workspace.resolve(asset);
        let type_candidates = asset_ref
            .extension
            .as_deref()
            .map(|extension| host.type_registry.find_by_extension(extension))
            .unwrap_or_default();
        println!("[ASSET] logical_path={}", asset_ref.logical_path.display());
        println!(
            "[ASSET] extension={}",
            asset_ref.extension.as_deref().unwrap_or("<none>")
        );
        println!("[TYPE-ROUTE] type_candidates={}", type_candidates.len());
        for type_descriptor in &type_candidates {
            println!(
                "[TYPE-ROUTE] type_id={} content_kind={} provider={}",
                type_descriptor.type_id,
                type_descriptor.content_kind,
                type_descriptor.provider_id.as_deref().unwrap_or("<none>")
            );
        }

        let selected_type = type_candidates
            .first()
            .copied()
            .ok_or_else(|| "no format type descriptor matched this asset".to_owned())?;
        let route = host
            .routes
            .route_for_format_type(selected_type)
            .ok_or_else(|| "no gateway route matched this format type".to_owned())?;
        println!(
            "[ROUTE] gateway={} provider={} active={} reason={} capabilities={}",
            route.gateway_id,
            route.provider_id,
            route.active,
            route.selection_reason,
            route.capability_ids.join(",")
        );
        let selected = host
            .capability_registry
            .providers()
            .iter()
            .find(|provider| provider.id == route.provider_id)
            .ok_or_else(|| format!("route selected missing provider '{}'", route.provider_id))?;

        println!("[ROUTE] selected_provider={}", selected.id);
        println!("[ROUTE] selected_kind={}", selected.kind);
        let selected_type = Some(selected_type);
        let preview = PreviewModel::from_route(selected, selected_type, &asset_ref);
        let inspector = InspectorModel::for_provider(selected, &asset_ref);
        let shell = EditorShellModel::compose(selected, &asset_ref, preview, inspector);
        shell.print_summary();

        Ok(())
    }
}
