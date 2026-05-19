use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use newengine_core::render::{
    BackendShadowCapabilities, LightExtractionProviderRequest, LightExtractionProviderResponse,
    LightExtractionSnapshot, LightPlanContribution, LightPlanContributionKind, RenderBoundsSnapshot,
    RenderViewSnapshot, RenderTargetId, ShadowSettingsSnapshot, TextureId,
};
use newengine_core::EngineResult;
use newengine_plugin_api::{
    Blob, CapabilityId, CapabilityKind, CapabilityRole, MethodName,
    CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER,
};
use newengine_plugin_host::{call_service_v1, has_service, PluginsSnapshot};
use newengine_lighting::ShadowMethod;
use newengine_math::Mat4;
use newengine_render_feature_api::{
    LightExtractionCommand, LightExtractionCtx, LightExtractionProvider, LightShadowPlan,
    LIGHT_PROVIDER_CAP_EXTRACTION,
};
use serde::Deserialize;

pub(super) const LIGHT_PROVIDER_TAG_PLUGIN: &str = "plugin";

static WARNED_PLUGIN_LIGHT_PROVIDER_BRIDGE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub(crate) struct ExternalLightExtractionProviderDesc {
    pub(super) id: String,
    pub(super) plugin_id: String,
    pub(super) label: String,
    pub(super) tags: Vec<String>,
    pub(super) capabilities: Vec<String>,
    pub(super) light_kinds: Vec<String>,
    pub(super) service_id: Option<String>,
    pub(super) method: String,
}


#[derive(Clone, Debug, Deserialize)]
struct PluginLightProviderJson {
    id: Option<String>,
    label: Option<String>,
    tags: Option<Vec<String>>,
    capabilities: Option<Vec<String>>,
    light_kinds: Option<Vec<String>>,
    service_id: Option<String>,
    method: Option<String>,
}

pub(crate) struct LightExtractionProviderRegistry {
    providers: Vec<Arc<dyn LightExtractionProvider>>,
    external_providers: Vec<ExternalLightExtractionProviderDesc>,
}

impl LightExtractionProviderRegistry {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            providers: Vec::new(),
            external_providers: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn from_runtime_providers(providers: Vec<Arc<dyn LightExtractionProvider>>) -> Self {
        Self {
            providers,
            external_providers: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn runtime_provider_arcs(&self) -> Vec<Arc<dyn LightExtractionProvider>> {
        self.providers.clone()
    }

    pub(crate) fn register_provider(&mut self, provider: Arc<dyn LightExtractionProvider>) {
        let id = provider.id();
        if self.providers.iter().any(|existing| existing.id() == id) {
            log::warn!(
                "render light extraction registry: duplicate runtime provider id='{}' ignored",
                id
            );
            return;
        }

        self.providers.push(provider);
    }

    pub(crate) fn register_external_provider(&mut self, provider: ExternalLightExtractionProviderDesc) {
        if self
            .external_providers
            .iter()
            .any(|existing| existing.plugin_id == provider.plugin_id && existing.id == provider.id)
        {
            return;
        }

        if provider.service_id.as_deref().is_none()
            && !WARNED_PLUGIN_LIGHT_PROVIDER_BRIDGE.swap(true, Ordering::Relaxed)
        {
            log::warn!(
                "render light extraction registry: plugin provider id='{}' plugin='{}' has no service_id; registered as descriptor-only",
                provider.id,
                provider.plugin_id
            );
        }

        self.external_providers.push(provider);
    }

    pub(crate) fn sync_plugin_capabilities(&mut self, snapshot: &PluginsSnapshot) {
        for plugin in snapshot.plugins.iter() {
            for capability in plugin.capabilities.iter() {
                if capability.role != CapabilityRole::Provides {
                    continue;
                }
                if capability.kind != CapabilityKind::SceneContributionV1
                    && capability.kind != CapabilityKind::ServiceV1
                {
                    continue;
                }
                if capability.id.as_str() != CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER {
                    continue;
                }

                match parse_plugin_light_provider(&plugin.id, capability.describe_json.as_str()) {
                    Some(provider) => self.register_external_provider(provider),
                    None => log::warn!(
                        "render light extraction registry: plugin='{}' capability='{}' has invalid provider metadata JSON",
                        plugin.id,
                        capability.id
                    ),
                }
            }
        }
    }

    pub(crate) fn extract_runtime_command(
        &self,
        ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightExtractionCommand>> {
        for provider in self.providers.iter() {
            if !provider.supports(ctx) {
                continue;
            }
            if let Some(command) = provider.extract(ctx)? {
                return Ok(Some(command));
            }
        }
        Ok(None)
    }

    pub(crate) fn extract_external_shadow_plan(
        &self,
        ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightShadowPlan>> {
        if self.external_providers.is_empty() {
            return Ok(None);
        }

        let request = build_light_provider_request(ctx);
        let payload = serde_json::to_vec(&request).map_err(|e| {
            newengine_core::EngineError::other(format!(
                "render light extraction provider request encode failed: {e}"
            ))
        })?;

        for provider in &self.external_providers {
            let Some(service_id) = provider.service_id.as_deref() else {
                continue;
            };
            if !has_service(service_id) {
                log::warn!(
                    "render light extraction registry: executable provider id='{}' plugin='{}' service='{}' is not registered yet",
                    provider.id,
                    provider.plugin_id,
                    service_id
                );
                continue;
            }

            let result = call_service_v1(
                CapabilityId::from(service_id),
                MethodName::from(provider.method.as_str()),
                Blob::from(payload.clone()),
            );
            let bytes = match result.into_result() {
                Ok(bytes) => bytes,
                Err(err) => {
                    log::warn!(
                        "render light extraction registry: provider id='{}' plugin='{}' service='{}' call failed: {}",
                        provider.id,
                        provider.plugin_id,
                        service_id,
                        err
                    );
                    continue;
                }
            };

            let response: LightExtractionProviderResponse = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    newengine_core::EngineError::other(format!(
                        "render light extraction provider '{}' returned invalid response JSON: {e}",
                        provider.id
                    ))
                })?;

            for warning in response.warnings {
                log::warn!("render light extraction provider '{}': {}", provider.id, warning);
            }

            let Some(contribution) = response.contribution else {
                continue;
            };
            for warning in &contribution.warnings {
                log::warn!("render light extraction provider '{}': {}", provider.id, warning);
            }
            if !contribution.handled {
                continue;
            }

            return Ok(Some(light_plan_from_contribution(ctx, contribution)));
        }

        Ok(None)
    }

    #[inline]
    pub(crate) fn labels(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.providers.len() + self.external_providers.len());
        for provider in self.providers.iter() {
            let metadata = provider.metadata();
            out.push(format!(
                "{}:'{}'[{} caps={}]",
                metadata.id,
                metadata.label,
                metadata.tags.join("|"),
                metadata.capabilities.join("|")
            ));
        }
        for provider in self.external_providers.iter() {
            out.push(format!(
                "{}@{}:'{}'[{} caps={} kinds={}]",
                provider.id,
                provider.plugin_id,
                provider.label,
                provider.tags.join("|"),
                provider.capabilities.join("|"),
                provider.light_kinds.join("|")
            ));
        }
        out
    }
}
