#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use newengine_core::render::{
    DrawListProviderExtractRequest, DrawListProviderExtractResponse, FrameGraphRoute,
    FrameGraphRoutes, RenderBoundsSnapshot, RenderCameraSnapshot, RenderDrawListKind,
    SceneExtractionSnapshot, VisibilityMask, Extent2D, RectI32, RenderApi, Viewport,
};
use newengine_core::EngineResult;
use newengine_math::Mat4;
use newengine_plugin_api::{
    Blob, CapabilityId, CapabilityKind, CapabilityRole, MethodName,
    CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER_V1, CAPABILITY_TAG_LEGACY,
};
use newengine_plugin_host::{call_service_v1, has_service, PluginsSnapshot};
use newengine_render_frame_graph::{DrawListDesc, DrawListRouteValidationReport};
use newengine_ui::draw::UiDrawList;
use serde::Deserialize;

use super::lights::PackedLights;
use super::scene::BoundsSnap;
use super::shadows::{LightShadowPlan, ShadowFrame};
use super::external_contribution_lowering::lower_external_draw_list_contribution;
use super::RuntimeRenderController;

pub(super) const PROVIDER_TAG_RUNTIME: &str = "runtime";
pub(super) const PROVIDER_TAG_BUILTIN: &str = "builtin";
pub(super) const PROVIDER_TAG_PLUGIN: &str = "plugin";
pub(super) const PROVIDER_TAG_LEGACY: &str = CAPABILITY_TAG_LEGACY;
pub(super) const PROVIDER_CAP_DRAW_LISTS: &str = CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER_V1;

const EMPTY_LISTS: &[RenderDrawListKind] = &[];
const OPAQUE_FORWARD: &[RenderDrawListKind] = &[RenderDrawListKind::OpaqueForward];
const SHADOW_AND_OPAQUE: &[RenderDrawListKind] = &[
    RenderDrawListKind::ShadowCasters,
    RenderDrawListKind::OpaqueForward,
];
const UI_LIST: &[RenderDrawListKind] = &[RenderDrawListKind::Ui];

static WARNED_LEGACY_DRAW_LIST_PROVIDER: AtomicBool = AtomicBool::new(false);
static WARNED_PLUGIN_PROVIDER_BRIDGE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
pub(super) struct RenderDrawListProviderMetadata {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) tags: &'static [&'static str],
    pub(super) capabilities: &'static [&'static str],
}

impl RenderDrawListProviderMetadata {
    #[inline]
    pub(super) fn runtime_builtin(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            tags: &[PROVIDER_TAG_RUNTIME, PROVIDER_TAG_BUILTIN],
            capabilities: &[PROVIDER_CAP_DRAW_LISTS],
        }
    }

    #[inline]
    pub(super) fn has_tag(self, tag: &str) -> bool {
        self.tags.iter().any(|it| *it == tag)
    }
}

#[derive(Clone, Debug)]
pub(super) struct ExternalRenderDrawListProviderDesc {
    pub(super) id: String,
    pub(super) plugin_id: String,
    pub(super) label: String,
    pub(super) tags: Vec<String>,
    pub(super) capabilities: Vec<String>,
    pub(super) draw_lists: Vec<RenderDrawListKind>,
    pub(super) service_id: Option<String>,
    pub(super) method: String,
}

impl ExternalRenderDrawListProviderDesc {
    #[inline]
    fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|it| it == tag)
    }
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

pub(super) struct RenderDrawListProviderRegistry {
    providers: Vec<Arc<dyn RenderDrawListProvider>>,
    external_providers: Vec<ExternalRenderDrawListProviderDesc>,
}

impl RenderDrawListProviderRegistry {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            providers: Vec::new(),
            external_providers: Vec::new(),
        }
    }

    pub(super) fn register_provider(&mut self, provider: Arc<dyn RenderDrawListProvider>) {
        let id = provider.id();
        if self.providers.iter().any(|existing| existing.id() == id) {
            log::warn!(
                "render draw-list provider registry: duplicate runtime provider id='{}' ignored",
                id
            );
            return;
        }

        let metadata = provider.metadata();
        if metadata.has_tag(PROVIDER_TAG_LEGACY) && !WARNED_LEGACY_DRAW_LIST_PROVIDER.swap(true, Ordering::Relaxed) {
            log::warn!(
                "render draw-list provider registry: provider id='{}' tag='legacy' -- migrate to Render API V3 provider contracts",
                metadata.id
            );
        }

        self.providers.push(provider);
    }

    pub(super) fn register_external_provider(&mut self, provider: ExternalRenderDrawListProviderDesc) {
        if self
            .external_providers
            .iter()
            .any(|existing| existing.plugin_id == provider.plugin_id && existing.id == provider.id)
        {
            return;
        }

        if provider.has_tag(PROVIDER_TAG_LEGACY) && !WARNED_LEGACY_DRAW_LIST_PROVIDER.swap(true, Ordering::Relaxed) {
            log::warn!(
                "render draw-list provider registry: plugin provider id='{}' plugin='{}' tag='legacy' -- migrate to current Render API V3 provider capability",
                provider.id,
                provider.plugin_id
            );
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
                if capability.id.as_str() != CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER_V1 {
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
    pub(super) fn providers(&self) -> Vec<&dyn RenderDrawListProvider> {
        self.providers
            .iter()
            .map(|provider| provider.as_ref() as &dyn RenderDrawListProvider)
            .collect()
    }

    pub(super) fn add_external_draw_lists(
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

    pub(super) fn extract_external_providers(
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

    pub(super) fn validate_routes(
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


#[inline]
fn parse_plugin_draw_list_provider(
    plugin_id: &str,
    describe_json: &str,
) -> Option<ExternalRenderDrawListProviderDesc> {
    let parsed: PluginDrawListProviderJson = serde_json::from_str(describe_json).ok()?;
    let id = parsed
        .id
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| format!("{plugin_id}.render_draw_lists"));
    let label = parsed
        .label
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let mut tags = parsed.tags.unwrap_or_default();
    push_unique_string(&mut tags, PROVIDER_TAG_PLUGIN);
    let mut capabilities = parsed.capabilities.unwrap_or_default();
    push_unique_string(&mut capabilities, PROVIDER_CAP_DRAW_LISTS);
    let mut draw_lists = Vec::new();
    for item in parsed.draw_lists.unwrap_or_default() {
        if let Some(kind) = parse_draw_list_kind(&item) {
            if !draw_lists.contains(&kind) {
                draw_lists.push(kind);
            }
        } else {
            log::warn!(
                "render draw-list provider registry: plugin='{}' provider='{}' declares unknown draw_list='{}'",
                plugin_id,
                id,
                item
            );
        }
    }
    let service_id = parsed.service_id.filter(|it| !it.trim().is_empty());
    let method = parsed
        .method
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| newengine_core::render::RENDER_DRAW_LIST_PROVIDER_METHOD_EXTRACT_V1.to_string());

    Some(ExternalRenderDrawListProviderDesc {
        id,
        plugin_id: plugin_id.to_string(),
        label,
        tags,
        capabilities,
        draw_lists,
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

#[inline]
fn parse_draw_list_kind(value: &str) -> Option<RenderDrawListKind> {
    match value.trim() {
        "shadow_casters" | "ShadowCasters" | "shadow" => Some(RenderDrawListKind::ShadowCasters),
        "opaque_forward" | "OpaqueForward" | "opaque" => Some(RenderDrawListKind::OpaqueForward),
        "transparent" | "Transparent" => Some(RenderDrawListKind::Transparent),
        "ui" | "Ui" | "UI" => Some(RenderDrawListKind::Ui),
        "debug" | "Debug" => Some(RenderDrawListKind::Debug),
        _ => None,
    }
}


#[inline]
fn build_draw_list_provider_request(
    ctx: &SceneExtractionCtx<'_>,
    lists: &RuntimeDrawListSet,
    frame_plan: &newengine_render_frame_graph::RenderFramePlan,
) -> DrawListProviderExtractRequest {
    let routes = FrameGraphRoutes {
        routes: frame_plan
            .graph
            .passes
            .iter()
            .map(|pass| FrameGraphRoute {
                pass: pass.kind,
                draw_lists: pass.draw_lists.clone(),
            })
            .collect(),
    };

    DrawListProviderExtractRequest {
        scene: SceneExtractionSnapshot {
            frame_index: frame_plan.graph.frame_index,
            viewport_extent: ctx.viewport_extent,
            surface_extent: ctx.surface_extent,
            runtime: ctx.runtime,
            editor_overlays: ctx.editor_overlays,
            bounds: RenderBoundsSnapshot {
                center: [ctx.bounds.center.x, ctx.bounds.center.y, ctx.bounds.center.z],
                radius: ctx.bounds.radius,
            },
            camera: RenderCameraSnapshot {
                view_projection_cols: ctx.viewproj.to_cols_array_2d(),
                position_ws: [ctx.rig.position.x, ctx.rig.position.y, ctx.rig.position.z],
            },
            active_draw_lists: lists.kinds().into_iter().collect(),
        },
        visibility: VisibilityMask {
            shadow_casters: ctx.visibility().shadow_casters,
            opaque_forward: ctx.visibility().opaque_forward,
            transparent: ctx.visibility().transparent,
            ui: ctx.visibility().ui,
            debug: ctx.visibility().debug,
        },
        routes,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeVisibilityPlan {
    pub(super) shadow_casters: bool,
    pub(super) opaque_forward: bool,
    pub(super) transparent: bool,
    pub(super) ui: bool,
    pub(super) debug: bool,
}

impl RuntimeVisibilityPlan {
    #[inline]
    pub(super) fn standard(shadow_casters: bool, ui: bool, debug: bool) -> Self {
        Self {
            shadow_casters,
            opaque_forward: true,
            transparent: false,
            ui,
            debug,
        }
    }

    #[inline]
    fn allows(self, kind: RenderDrawListKind) -> bool {
        match kind {
            RenderDrawListKind::ShadowCasters => self.shadow_casters,
            RenderDrawListKind::OpaqueForward => self.opaque_forward,
            RenderDrawListKind::Transparent => self.transparent,
            RenderDrawListKind::Ui => self.ui,
            RenderDrawListKind::Debug => self.debug,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SceneExtractionCtx<'a> {
    pub(super) scene: &'a newengine_scene::Scene,
    pub(super) lit: super::super::gpu::LitPipeline,
    pub(super) viewproj: Mat4,
    pub(super) rig: &'a newengine_camera::CameraRig,
    pub(super) bounds: BoundsSnap,
    pub(super) lights: PackedLights,
    pub(super) shadow_plan: LightShadowPlan,
    pub(super) shadow_frame: ShadowFrame,
    pub(super) viewport_extent: Extent2D,
    pub(super) surface_extent: Extent2D,
    pub(super) runtime: bool,
    pub(super) editor_overlays: bool,
    pub(super) ui: Option<&'a UiDrawList>,
}

impl<'a> SceneExtractionCtx<'a> {
    #[inline]
    pub(super) fn visibility(&self) -> RuntimeVisibilityPlan {
        RuntimeVisibilityPlan::standard(
            self.shadow_plan.is_active(),
            self.ui.is_some(),
            self.editor_overlays,
        )
    }
}

pub(super) trait RenderDrawListProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::runtime_builtin(self.id(), self.id())
    }

    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind];

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()>;
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeDrawListSet {
    lists: Vec<RuntimeDrawList>,
}

impl RuntimeDrawListSet {
    pub(super) fn extract(
        visibility: RuntimeVisibilityPlan,
        ctx: &SceneExtractionCtx<'_>,
        providers: &[&dyn RenderDrawListProvider],
    ) -> Self {
        let mut this = Self { lists: Vec::with_capacity(5) };
        for provider in providers {
            for &kind in provider.provided_draw_lists(ctx) {
                if visibility.allows(kind) {
                    this.push(kind);
                }
            }
        }
        this
    }

    #[inline]
    pub(super) fn descriptors(&self) -> Vec<DrawListDesc> {
        self.lists
            .iter()
            .map(|list| DrawListDesc::standard(list.kind))
            .collect()
    }

    #[inline]
    pub(super) fn kinds(&self) -> BTreeSet<RenderDrawListKind> {
        self.lists.iter().map(|list| list.kind).collect()
    }

    #[inline]
    pub(super) fn contains(&self, kind: RenderDrawListKind) -> bool {
        self.lists.iter().any(|list| list.kind == kind)
    }

    pub(super) fn record_pass_state(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if self.contains(RenderDrawListKind::ShadowCasters) && ctx.shadow_plan.is_active() {
            let extent = ctx.shadow_plan.extent();
            let _ = out.record(RenderDrawListKind::ShadowCasters, move |_this, r| {
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(
                    0,
                    0,
                    extent.width as i32,
                    extent.height as i32,
                ))?;
                Ok(())
            })?;
        }

        if self.contains(RenderDrawListKind::OpaqueForward) {
            let extent = ctx.viewport_extent;
            let _ = out.record(RenderDrawListKind::OpaqueForward, move |_this, r| {
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(
                    0,
                    0,
                    extent.width as i32,
                    extent.height as i32,
                ))?;
                Ok(())
            })?;
        }

        Ok(())
    }

    #[inline]
    pub(super) fn push(&mut self, kind: RenderDrawListKind) {
        if !self.contains(kind) {
            self.lists.push(RuntimeDrawList::new(kind));
        }
    }
}

pub(super) struct DrawListBuildCtx<'a> {
    controller: &'a mut RuntimeRenderController,
    render: &'a mut dyn RenderApi,
    lists: &'a RuntimeDrawListSet,
}

impl<'a> DrawListBuildCtx<'a> {
    #[inline]
    pub(super) fn new(
        controller: &'a mut RuntimeRenderController,
        render: &'a mut dyn RenderApi,
        lists: &'a RuntimeDrawListSet,
    ) -> Self {
        Self {
            controller,
            render,
            lists,
        }
    }

    pub(super) fn record<T>(
        &mut self,
        kind: RenderDrawListKind,
        record: impl FnOnce(&mut RuntimeRenderController, &mut dyn RenderApi) -> EngineResult<T>,
    ) -> EngineResult<Option<T>> {
        if !self.lists.contains(kind) {
            return Ok(None);
        }

        let controller = &mut *self.controller;
        let render = &mut *self.render;
        let value = super::record_draw_list(render, kind, |r| record(controller, r))?;
        Ok(Some(value))
    }
}

#[derive(Clone, Debug)]
struct RuntimeDrawList {
    kind: RenderDrawListKind,
}

impl RuntimeDrawList {
    #[inline]
    fn new(kind: RenderDrawListKind) -> Self {
        Self { kind }
    }
}

#[inline]
pub(super) const fn opaque_forward_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { OPAQUE_FORWARD } else { EMPTY_LISTS }
}

#[inline]
pub(super) const fn shadow_and_opaque_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { SHADOW_AND_OPAQUE } else { OPAQUE_FORWARD }
}

#[inline]
pub(super) const fn ui_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { UI_LIST } else { EMPTY_LISTS }
}
