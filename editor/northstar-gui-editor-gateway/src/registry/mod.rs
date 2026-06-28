use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use northstar_gui_editor_assets::format_types::FormatTypeDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: PathBuf,
    pub capabilities: Vec<String>,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayRoute {
    pub gateway_id: String,
    pub provider_id: String,
    pub capability_ids: Vec<String>,
    pub active: bool,
    pub selection_reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRegistry {
    routes_by_type: BTreeMap<String, GatewayRoute>,
}

impl GatewayRegistry {
    pub fn from_format_types(capability_registry: &CapabilityRegistry, format_types: &[FormatTypeDescriptor]) -> Self {
        let routes_by_type = format_types
            .iter()
            .filter_map(|format_type| {
                capability_registry
                    .resolve_route(format_type)
                    .map(|route| (format_type.type_id.clone(), route))
            })
            .collect();
        Self { routes_by_type }
    }

    pub fn routes(&self) -> Vec<&GatewayRoute> {
        self.routes_by_type.values().collect()
    }

    pub fn route_count(&self) -> usize {
        self.routes_by_type.len()
    }

    pub fn contains_type(&self, type_id: &str) -> bool {
        self.routes_by_type.contains_key(type_id)
    }

    pub fn route_for_type_id(&self, type_id: &str) -> Option<&GatewayRoute> {
        self.routes_by_type.get(type_id)
    }

    pub fn route_for_format_type(&self, format_type: &FormatTypeDescriptor) -> Option<&GatewayRoute> {
        self.route_for_type_id(&format_type.type_id)
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    providers: Vec<ProviderDescriptor>,
    capability_index: BTreeMap<String, Vec<usize>>,
}

impl CapabilityRegistry {
    pub fn from_providers(providers: Vec<ProviderDescriptor>) -> Self {
        let mut capability_index: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, provider) in providers.iter().enumerate() {
            for capability in &provider.capabilities {
                capability_index
                    .entry(capability.to_owned())
                    .or_default()
                    .push(index);
            }
        }

        Self {
            providers,
            capability_index,
        }
    }

    pub fn providers(&self) -> &[ProviderDescriptor] {
        &self.providers
    }

    pub fn capability_count(&self) -> usize {
        self.capability_index.len()
    }

    pub fn providers_with_capability_suffix(&self, suffix: &str) -> Vec<&ProviderDescriptor> {
        let mut seen = BTreeSet::new();
        let mut providers = Vec::new();
        for (capability, indexes) in &self.capability_index {
            if capability.ends_with(suffix) || capability == suffix.trim_start_matches('.') {
                for &index in indexes {
                    if seen.insert(index) {
                        providers.push(&self.providers[index]);
                    }
                }
            }
        }
        providers
    }

    pub fn providers_with_capability_fragment(&self, fragment: &str) -> Vec<&ProviderDescriptor> {
        let mut seen = BTreeSet::new();
        let mut providers = Vec::new();
        for (capability, indexes) in &self.capability_index {
            if capability.contains(fragment) {
                for &index in indexes {
                    if seen.insert(index) {
                        providers.push(&self.providers[index]);
                    }
                }
            }
        }
        providers
    }

    pub fn providers_for_format_type(&self, format_type: &FormatTypeDescriptor) -> Vec<&ProviderDescriptor> {
        let mut ranked = Vec::new();
        let mut seen = BTreeSet::new();

        if let Some(provider_id) = format_type.provider_id.as_deref() {
            for (index, provider) in self.providers.iter().enumerate() {
                if provider.id == provider_id && seen.insert(index) {
                    ranked.push(provider);
                }
            }
        }

        for capability in required_capabilities(format_type) {
            if let Some(indexes) = self.capability_index.get(&capability) {
                for &index in indexes {
                    if seen.insert(index) {
                        ranked.push(&self.providers[index]);
                    }
                }
            }
        }

        ranked.sort_by(|a, b| provider_rank(b).cmp(&provider_rank(a)).then_with(|| a.id.cmp(&b.id)));
        ranked
    }

    pub fn resolve_route(&self, format_type: &FormatTypeDescriptor) -> Option<GatewayRoute> {
        let required = required_capabilities(format_type);
        let providers = self.providers_for_format_type(format_type);
        let selected = providers.first()?;
        Some(GatewayRoute {
            gateway_id: gateway_id_for_format_type(format_type),
            provider_id: selected.id.clone(),
            capability_ids: required,
            active: true,
            selection_reason: route_selection_reason(format_type, selected),
        })
    }
}

fn provider_rank(provider: &ProviderDescriptor) -> u32 {
    let mut score = 0;
    if provider.kind.contains("codec") {
        score += 100;
    }
    if provider.capabilities.iter().any(|capability| capability.ends_with(".read")) {
        score += 50;
    }
    if provider.capabilities.iter().any(|capability| capability.contains("preview.")) {
        score += 20;
    }
    if provider.capabilities.iter().any(|capability| capability.ends_with(".edit_schema")) {
        score += 10;
    }
    score
}

fn required_capabilities(format_type: &FormatTypeDescriptor) -> Vec<String> {
    let mut capabilities = Vec::new();
    if format_type.capabilities.can_read {
        capabilities.push("asset.read".to_owned());
    }
    if format_type.capabilities.can_inspect {
        capabilities.push("asset.inspect".to_owned());
    }
    if format_type.capabilities.can_preview {
        if let Some(surface) = format_type.preview_surface.as_deref() {
            capabilities.push(format!("asset.preview.{}", surface.trim().trim_start_matches("preview.")));
        } else {
            capabilities.push("asset.preview".to_owned());
        }
    }
    if format_type.capabilities.can_validate {
        capabilities.push("asset.validate".to_owned());
    }
    capabilities
}

fn gateway_id_for_format_type(format_type: &FormatTypeDescriptor) -> String {
    if format_type.capabilities.can_preview {
        return "editor.preview".to_owned();
    }
    if format_type.capabilities.can_inspect {
        return "editor.inspector".to_owned();
    }
    if format_type.capabilities.can_read {
        return "editor.assets.read".to_owned();
    }
    "editor.assets".to_owned()
}

fn route_selection_reason(format_type: &FormatTypeDescriptor, provider: &ProviderDescriptor) -> String {
    if format_type.provider_id.as_deref() == Some(provider.id.as_str()) {
        return "format type declares provider_id".to_owned();
    }
    "provider selected by declared capabilities for format type".to_owned()
}
