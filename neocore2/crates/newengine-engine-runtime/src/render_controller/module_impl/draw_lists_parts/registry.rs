use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use newengine_core::render::{
    DrawListProviderExtractRequest, DrawListProviderExtractResponse, FrameGraphRoute,
    FrameGraphRoutes, RenderBoundsSnapshot, RenderViewSnapshot, RenderDrawListKind,
    SceneExtractionSnapshot, VisibilityMask,
};
use newengine_core::EngineResult;
use newengine_plugin_api::{
    Blob, CapabilityId, CapabilityKind, CapabilityRole, MethodName,
    CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER,
};
use newengine_plugin_host::{call_service_v1, has_service, PluginsSnapshot};
use newengine_render_feature_api::{
    RenderDrawListProvider, RuntimeVisibilityPlan, SceneExtractionCtx, PROVIDER_CAP_DRAW_LISTS,
};
use newengine_render_frame_graph::DrawListRouteValidationReport;
use serde::Deserialize;

use super::external_contribution_lowering::lower_external_draw_list_contribution;

pub(super) const PROVIDER_TAG_PLUGIN: &str = "plugin";

static WARNED_PLUGIN_PROVIDER_BRIDGE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub(crate) struct ExternalRenderDrawListProviderDesc {
    pub(super) id: String,
    pub(super) plugin_id: String,
    pub(super) label: String,
    pub(super) tags: Vec<String>,
    pub(super) capabilities: Vec<String>,
    pub(super) draw_lists: Vec<RenderDrawListKind>,
    pub(super) service_id: Option<String>,
    pub(super) method: String,
}


#[derive(Clone, Debug, Deserialize)]
struct PluginDrawListProviderJson {
    id: Option<String>,
    label: Option<String>,
    tags: Option<Vec<String>>,
    capabilities: Option<Vec<String>>,
    draw_lists: Option<Vec<String>>,
    service_id: Option<String>,
    method: Option<String>,
}

pub(crate) struct RenderDrawListProviderRegistry {
    providers: Vec<Arc<dyn RenderDrawListProvider>>,
    external_providers: Vec<ExternalRenderDrawListProviderDesc>,
}

impl RenderDrawListProviderRegistry {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            providers: Vec::new(),
            external_providers: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn from_runtime_providers(providers: Vec<Arc<dyn RenderDrawListProvider>>) -> Self {
        Self {
            providers,
            external_providers: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn runtime_provider_arcs(&self) -> Vec<Arc<dyn RenderDrawListProvider>> {
        self.providers.clone()
    }

    pub(crate) fn register_provider(&mut self, provider: Arc<dyn RenderDrawListProvider>) {
        let id = provider.id();
        if self.providers.iter().any(|existing| existing.id() == id) {
            log::warn!(
                "render draw-list provider registry: duplicate runtime provider id='{}' ignored",
                id
            );
            return;
        }

        self.providers.push(provider);
    }

    pub(crate) fn register_external_provider(&mut self, provider: ExternalRenderDrawListProviderDesc) {
        if self
            .external_providers
            .iter()
            .any(|existing| existing.plugin_id == provider.plugin_id && existing.id == provider.id)
        {
            return;
        }

        if provider.service_id.as_deref().is_none()
            && !WARNED_PLUGIN_PROVIDER_BRIDGE.swap(true, Ordering::Relaxed)
        {
            log::warn!(
                "render draw-list provider registry: plugin provider id='{}' plugin='{}' has no service_id; registered as descriptor-only",
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
                if capability.id.as_str() != CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER {
                    continue;
                }

                match parse_plugin_draw_list_provider(&plugin.id, capability.describe_json.as_str()) {
                    Some(provider) => self.register_external_provider(provider),
                    None => log::warn!(
                        "render draw-list provider registry: plugin='{}' capability='{}' has invalid provider metadata JSON",
                        plugin.id,
                        capability.id
                    ),
                }
            }
        }
    }

    #[inline]
    pub(crate) fn providers(&self) -> Vec<&dyn RenderDrawListProvider> {
        self.providers
            .iter()
            .map(|provider| provider.as_ref() as &dyn RenderDrawListProvider)
            .collect()
    }

    pub(crate) fn add_external_draw_lists(
        &self,
        visibility: RuntimeVisibilityPlan,
        out: &mut RuntimeDrawListSet,
    ) {
        for provider in &self.external_providers {
            for &kind in &provider.draw_lists {
                if visibility.allows(kind) {
                    out.push(kind);
                }
            }
        }
    }

    pub(crate) fn extract_external_providers(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        lists: &RuntimeDrawListSet,
        frame_plan: &newengine_render_frame_graph::RenderFramePlan,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if self.external_providers.is_empty() {
            return Ok(());
        }

        let request = build_draw_list_provider_request(ctx, lists, frame_plan);
        let payload = serde_json::to_vec(&request).map_err(|e| {
            newengine_core::EngineError::other(format!(
                "render draw-list provider request encode failed: {e}"
            ))
        })?;

        for provider in &self.external_providers {
            let Some(service_id) = provider.service_id.as_deref() else {
                continue;
            };
            if !has_service(service_id) {
                log::warn!(
                    "render draw-list provider registry: executable provider id='{}' plugin='{}' service='{}' is not registered yet",
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
                        "render draw-list provider registry: provider id='{}' plugin='{}' service='{}' call failed: {}",
                        provider.id,
                        provider.plugin_id,
                        service_id,
                        err
                    );
                    continue;
                }
            };

            let response: DrawListProviderExtractResponse = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    newengine_core::EngineError::other(format!(
                        "render draw-list provider '{}' returned invalid response JSON: {e}",
                        provider.id
                    ))
                })?;

            for warning in response.warnings {
                log::warn!(
                    "render draw-list provider '{}': {}",
                    provider.id,
                    warning
                );
            }
            for contribution in response.contributions {
                if !lists.contains(contribution.draw_list) {
                    log::warn!(
                        "render draw-list provider '{}' contributed unrouted/inactive draw-list '{}'",
                        provider.id,
                        contribution.draw_list.label()
                    );
                    continue;
                }
                let lowering = lower_external_draw_list_contribution(provider, contribution, ctx, out)?;
                log::debug!(
                    "render draw-list provider '{}' lowered draw_list={} commands={} draw_calls={} skipped={} triangles={}",
                    provider.id,
                    lowering.draw_list.label(),
                    lowering.commands,
                    lowering.draw_calls,
                    lowering.skipped_commands,
                    lowering.triangle_count
                );
            }
        }

        Ok(())
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
                "{}@{}:'{}'[{} caps={}]",
                provider.id,
                provider.plugin_id,
                provider.label,
                provider.tags.join("|"),
                provider.capabilities.join("|")
            ));
        }
        out
    }

    pub(crate) fn validate_routes(
        &self,
        report: &DrawListRouteValidationReport,
    ) -> EngineResult<()> {
        for issue in &report.warnings {
            log::warn!(
                "render draw-list route validation: code='{}' {}",
                issue.code,
                issue.message
            );
        }
        if report.errors.is_empty() {
            return Ok(());
        }
        let message = report
            .errors
            .iter()
            .map(|issue| format!("{}: {}", issue.code, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        Err(newengine_core::EngineError::other(format!(
            "render draw-list route validation failed: {message}"
        )))
    }
}
