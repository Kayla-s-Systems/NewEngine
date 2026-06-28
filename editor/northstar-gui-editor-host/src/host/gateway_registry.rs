use northstar_gui_editor_gateway::registry::ProviderDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayRouteDto {
    pub gateway_id: String,
    pub service_kind: String,
    pub provider_id: String,
    pub capability_ids: Vec<String>,
    pub active: bool,
    pub selection_reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRouteRegistry {
    routes: Vec<GatewayRouteDto>,
}

impl GatewayRouteRegistry {
    pub fn from_providers(providers: &[ProviderDescriptor]) -> Self {
        let mut routes = Vec::new();
        for provider in providers {
            let service_kind = service_kind_from_provider(provider);
            let gateway_id = gateway_from_service_kind(&service_kind);
            routes.push(GatewayRouteDto {
                gateway_id,
                service_kind,
                provider_id: provider.id.clone(),
                capability_ids: provider.capabilities.clone(),
                active: true,
                selection_reason: "descriptor-discovered provider route".to_owned(),
            });
        }
        routes.sort_by(|a, b| a.gateway_id.cmp(&b.gateway_id).then_with(|| a.provider_id.cmp(&b.provider_id)));
        Self { routes }
    }

    pub fn routes(&self) -> &[GatewayRouteDto] {
        &self.routes
    }
}

fn service_kind_from_provider(provider: &ProviderDescriptor) -> String {
    if provider.capabilities.iter().any(|capability| capability.starts_with("asset.preview") || capability.contains("preview")) {
        return "preview".to_owned();
    }
    if provider.capabilities.iter().any(|capability| capability.contains("format") || capability.contains("pack") || capability.contains("extract")) {
        return "asset_format".to_owned();
    }
    if provider.kind.contains("codec") {
        return "codec".to_owned();
    }
    if provider.kind.contains("tool") {
        return "tool".to_owned();
    }
    provider.kind.clone()
}

fn gateway_from_service_kind(service_kind: &str) -> String {
    match service_kind {
        "preview" => "editor.preview".to_owned(),
        "asset_format" | "codec" => "editor.assets.format".to_owned(),
        "tool" => "editor.tools".to_owned(),
        other => format!("editor.{other}"),
    }
}
