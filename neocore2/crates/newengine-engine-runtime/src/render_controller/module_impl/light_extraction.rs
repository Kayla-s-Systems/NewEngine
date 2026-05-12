#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use newengine_core::render::{
    BackendShadowCapabilities, Extent2D, LightExtractionProviderRequest,
    LightExtractionProviderResponse, LightExtractionSnapshot, LightPlanContribution,
    LightPlanContributionKind, RenderApi, RenderBoundsSnapshot, RenderCameraSnapshot,
    RenderTargetId, ShadowSettingsSnapshot, TextureId,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_plugin_api::{
    Blob, CapabilityId, CapabilityKind, CapabilityRole, MethodName,
    CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER_V1, CAPABILITY_TAG_LEGACY,
};
use newengine_plugin_host::{call_service_v1, has_service, PluginsSnapshot};
use newengine_math::Mat4;
use serde::Deserialize;

use super::scene::BoundsSnap;
use super::shadows::LightShadowPlan;
use super::RuntimeRenderController;

pub(super) const LIGHT_PROVIDER_TAG_RUNTIME: &str = "runtime";
pub(super) const LIGHT_PROVIDER_TAG_BUILTIN: &str = "builtin";
pub(super) const LIGHT_PROVIDER_TAG_PLUGIN: &str = "plugin";
pub(super) const LIGHT_PROVIDER_TAG_LEGACY: &str = CAPABILITY_TAG_LEGACY;
pub(super) const LIGHT_PROVIDER_CAP_EXTRACTION: &str = CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER_V1;

static WARNED_LEGACY_LIGHT_PROVIDER: AtomicBool = AtomicBool::new(false);
static WARNED_PLUGIN_LIGHT_PROVIDER_BRIDGE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
pub(super) struct LightExtractionProviderMetadata {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) tags: &'static [&'static str],
    pub(super) capabilities: &'static [&'static str],
}

impl LightExtractionProviderMetadata {
    #[inline]
    pub(super) fn runtime_builtin(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            tags: &[LIGHT_PROVIDER_TAG_RUNTIME, LIGHT_PROVIDER_TAG_BUILTIN],
            capabilities: &[LIGHT_PROVIDER_CAP_EXTRACTION],
        }
    }

    #[inline]
    pub(super) fn has_tag(self, tag: &str) -> bool {
        self.tags.iter().any(|it| *it == tag)
    }
}

pub(super) struct LightExtractionCtx<'a> {
    pub(super) controller: &'a mut RuntimeRenderController,
    pub(super) render: &'a mut dyn RenderApi,
    pub(super) world: &'a newengine_ecs::World,
    pub(super) bounds: BoundsSnap,
    pub(super) lit: super::super::gpu::LitPipeline,
    pub(super) settings: ShadowSettings,
    pub(super) frame_index: u64,
    pub(super) viewproj: Mat4,
    pub(super) camera_position: [f32; 3],
    pub(super) viewport_extent: Extent2D,
    pub(super) surface_extent: Extent2D,
}

impl<'a> LightExtractionCtx<'a> {
    #[inline]
    pub(super) fn new(
        controller: &'a mut RuntimeRenderController,
        render: &'a mut dyn RenderApi,
        world: &'a newengine_ecs::World,
        bounds: BoundsSnap,
        lit: super::super::gpu::LitPipeline,
        settings: ShadowSettings,
        frame_index: u64,
        viewproj: Mat4,
        camera_position: [f32; 3],
        viewport_extent: Extent2D,
        surface_extent: Extent2D,
    ) -> Self {
        Self {
            controller,
            render,
            world,
            bounds,
            lit,
            settings,
            frame_index,
            viewproj,
            camera_position,
            viewport_extent,
            surface_extent,
        }
    }
}

pub(super) trait LightExtractionProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::runtime_builtin(self.id(), self.id())
    }

    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool;

    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<LightShadowPlan>>;
}

#[derive(Clone, Debug)]
pub(super) struct ExternalLightExtractionProviderDesc {
    pub(super) id: String,
    pub(super) plugin_id: String,
    pub(super) label: String,
    pub(super) tags: Vec<String>,
    pub(super) capabilities: Vec<String>,
    pub(super) light_kinds: Vec<String>,
    pub(super) service_id: Option<String>,
    pub(super) method: String,
}

impl ExternalLightExtractionProviderDesc {
    #[inline]
    fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|it| it == tag)
    }
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

pub(super) struct LightExtractionProviderRegistry {
    providers: Vec<Arc<dyn LightExtractionProvider>>,
    external_providers: Vec<ExternalLightExtractionProviderDesc>,
}

impl LightExtractionProviderRegistry {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            providers: Vec::new(),
            external_providers: Vec::new(),
        }
    }

    pub(super) fn register_provider(&mut self, provider: Arc<dyn LightExtractionProvider>) {
        let id = provider.id();
        if self.providers.iter().any(|existing| existing.id() == id) {
            log::warn!(
                "render light extraction registry: duplicate runtime provider id='{}' ignored",
                id
            );
            return;
        }

        let metadata = provider.metadata();
        if metadata.has_tag(LIGHT_PROVIDER_TAG_LEGACY)
            && !WARNED_LEGACY_LIGHT_PROVIDER.swap(true, Ordering::Relaxed)
        {
            log::warn!(
                "render light extraction registry: provider id='{}' tag='legacy' -- migrate to Render API V3 light extraction contracts",
                metadata.id
            );
        }

        self.providers.push(provider);
    }

    pub(super) fn register_external_provider(&mut self, provider: ExternalLightExtractionProviderDesc) {
        if self
            .external_providers
            .iter()
            .any(|existing| existing.plugin_id == provider.plugin_id && existing.id == provider.id)
        {
            return;
        }

        if provider.has_tag(LIGHT_PROVIDER_TAG_LEGACY)
            && !WARNED_LEGACY_LIGHT_PROVIDER.swap(true, Ordering::Relaxed)
        {
            log::warn!(
                "render light extraction registry: plugin provider id='{}' plugin='{}' tag='legacy' -- migrate to current Render API V3 light extraction capability",
                provider.id,
                provider.plugin_id
            );
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

    pub(super) fn sync_plugin_capabilities(&mut self, snapshot: &PluginsSnapshot) {
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
                if capability.id.as_str() != CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER_V1 {
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

    pub(super) fn extract_shadow_plan(
        &self,
        ctx: &mut LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightShadowPlan>> {
        if let Some(plan) = self.extract_external_shadow_plan(ctx)? {
            return Ok(Some(plan));
        }

        for provider in self.providers.iter() {
            if !provider.supports(ctx) {
                continue;
            }
            if let Some(plan) = provider.extract(ctx)? {
                return Ok(Some(plan));
            }
        }
        Ok(None)
    }

    fn extract_external_shadow_plan(
        &self,
        ctx: &mut LightExtractionCtx<'_>,
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
    pub(super) fn labels(&self) -> Vec<String> {
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

#[inline]
fn build_light_provider_request(ctx: &LightExtractionCtx<'_>) -> LightExtractionProviderRequest {
    LightExtractionProviderRequest {
        scene: LightExtractionSnapshot {
            frame_index: ctx.frame_index,
            viewport_extent: ctx.viewport_extent,
            surface_extent: ctx.surface_extent,
            bounds: RenderBoundsSnapshot {
                center: [ctx.bounds.center.x, ctx.bounds.center.y, ctx.bounds.center.z],
                radius: ctx.bounds.radius,
            },
            camera: RenderCameraSnapshot {
                view_projection_cols: ctx.viewproj.to_cols_array_2d(),
                position_ws: ctx.camera_position,
            },
        },
        settings: ShadowSettingsSnapshot {
            enabled: ctx.settings.enabled,
            method: shadow_method_label(ctx.settings.method).to_string(),
            resolution: ctx.settings.resolution,
            max_distance: ctx.settings.max_distance,
            bias: ctx.settings.bias,
            softness: ctx.settings.softness,
            contact_strength: ctx.settings.contact_strength,
        },
        backend: BackendShadowCapabilities {
            directional_depth_map: true,
            cascaded_shadow_maps: false,
            point_cube_map: false,
            spot_depth_map: false,
            max_shadow_resolution: ctx.settings.resolution,
        },
    }
}

#[inline]
fn light_plan_from_contribution(
    ctx: &LightExtractionCtx<'_>,
    contribution: LightPlanContribution,
) -> LightShadowPlan {
    let resolution = contribution.resolution.max(1);
    let fallback = ctx.lit.white_texture;
    let kind = match contribution.kind {
        LightPlanContributionKind::Directional => super::shadows::ShadowLightKind::Directional,
        LightPlanContributionKind::Point => super::shadows::ShadowLightKind::Point,
        LightPlanContributionKind::Spot => super::shadows::ShadowLightKind::Spot,
        LightPlanContributionKind::AmbientOcclusion | LightPlanContributionKind::None => {
            return LightShadowPlan::disabled(fallback);
        }
    };

    if !contribution.supported {
        return LightShadowPlan::unsupported(kind, fallback, resolution);
    }

    let (Some(rt), Some(tex)) = (contribution.render_target, contribution.shadow_texture) else {
        return LightShadowPlan::unsupported(kind, fallback, resolution);
    };

    let rt = RenderTargetId::new(rt);
    let tex = TextureId::new(tex);
    let mvp = Mat4::from_cols_array_2d(&contribution.light_mvp_cols);

    match kind {
        super::shadows::ShadowLightKind::Directional => {
            LightShadowPlan::directional(rt, tex, resolution, mvp, contribution.params)
        }
        super::shadows::ShadowLightKind::Point | super::shadows::ShadowLightKind::Spot => {
            LightShadowPlan::unsupported(kind, fallback, resolution)
        }
    }
}

#[inline]
const fn shadow_method_label(method: ShadowMethod) -> &'static str {
    match method {
        ShadowMethod::None => "none",
        ShadowMethod::Auto => "auto",
        ShadowMethod::DirectionalDepthMap => "directional_depth_map",
        ShadowMethod::PointCubeMap => "point_cube_map",
        ShadowMethod::SpotDepthMap => "spot_depth_map",
    }
}

#[inline]
fn parse_plugin_light_provider(
    plugin_id: &str,
    describe_json: &str,
) -> Option<ExternalLightExtractionProviderDesc> {
    let parsed: PluginLightProviderJson = serde_json::from_str(describe_json).ok()?;
    let id = parsed
        .id
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| format!("{plugin_id}.light_extraction"));
    let label = parsed
        .label
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let mut tags = parsed.tags.unwrap_or_default();
    push_unique_string(&mut tags, LIGHT_PROVIDER_TAG_PLUGIN);
    let mut capabilities = parsed.capabilities.unwrap_or_default();
    push_unique_string(&mut capabilities, LIGHT_PROVIDER_CAP_EXTRACTION);
    let service_id = parsed.service_id.filter(|it| !it.trim().is_empty());
    let method = parsed
        .method
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| newengine_core::render::RENDER_LIGHT_EXTRACTION_PROVIDER_METHOD_EXTRACT_V1.to_string());

    Some(ExternalLightExtractionProviderDesc {
        id,
        plugin_id: plugin_id.to_string(),
        label,
        tags,
        capabilities,
        light_kinds: parsed.light_kinds.unwrap_or_default(),
        service_id,
        method,
    })
}

#[inline]
fn push_unique_string(dst: &mut Vec<String>, value: &str) {
    if !dst.iter().any(|it| it == value) {
        dst.push(value.to_string());
    }
}
