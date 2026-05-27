#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_core::host_events::CursorState;
use newengine_assets::{AssetResult, RuntimeTextureAsset};
use newengine_core::render::{Extent2D, RenderHardwareTier, RenderTargetId, SamplerId, TextureId};
use newengine_core::JobTicket;
use newengine_math::collections::FxHashMap;
use std::collections::VecDeque;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::{
    DebugLineGpu, MaterialGpuPipeline, MaterialGpuPipelineKey, MaterialGpuRegistry,
    LitPipeline, MaterialPipelineBuildProfile, PrimitiveGpu,
};
use super::material_bindings::MaterialTextureGpuResidency;
use super::metrics::RuntimeOverlayMetrics;
use super::module_impl::instancing::InstanceBufferUploader;
use super::module_impl::draw_lists::RenderDrawListProviderRegistry;
use super::module_impl::light_extraction::LightExtractionProviderRegistry;
use super::resource_lifetime::RenderGpuLifetimeQueue;
use super::runtime_profile::RenderRuntimeProfile;

type PrimGpuCache = FxHashMap<newengine_primitives::PrimitiveId, PrimitiveGpu>;
type TerrainGpuCache = FxHashMap<u64, PrimitiveGpu>;


#[derive(Clone, Copy)]
pub struct PerDrawUbo {
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub base_texture: TextureId,
    pub normal_texture: TextureId,
    pub roughness_texture: TextureId,
    pub shadow_texture: TextureId,
    pub sampler: SamplerId,
    pub last_seen_frame: u64,
}


/// Profile-owned render feature providers registered by the app/runtime profile.
///
/// The reusable engine render controller starts with empty registries. GameReady,
/// editor preview, tests or future content plugins must explicitly register the
/// draw-list and light extraction providers they need.
pub(super) struct RenderFeatureProviderState {
    pub(super) draw_list_providers: RenderDrawListProviderRegistry,
    pub(super) light_extraction_providers: LightExtractionProviderRegistry,
}

impl RenderFeatureProviderState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            draw_list_providers: RenderDrawListProviderRegistry::new(),
            light_extraction_providers: LightExtractionProviderRegistry::new(),
        }
    }
}

/// Stable bridge references shared by render runtime subsystems.
///
/// This keeps host/plugin/scene access outside the GPU backend adapter and makes
/// the controller a composition root instead of a god-object.
pub(super) struct RenderBridgeState {
    pub(super) viewport: Arc<ViewportBridge>,
    pub(super) plugins: Arc<PluginManagerBridge>,
    pub(super) scene: Arc<SceneBridge>,
}

impl RenderBridgeState {
    #[inline]
    pub(super) fn new(
        viewport: Arc<ViewportBridge>,
        plugins: Arc<PluginManagerBridge>,
        scene: Arc<SceneBridge>,
    ) -> Self {
        Self {
            viewport,
            plugins,
            scene,
        }
    }
}

/// Window/surface/viewport state owned by the engine-facing render runtime.
pub(super) struct RenderViewportState {
    pub(super) clear_color: [f32; 4],
    pub(super) last_w: u32,
    pub(super) last_h: u32,
    pub(super) last_vp_w: u32,
    pub(super) last_vp_h: u32,
    pub(super) last_aspect: f32,
    pub(super) pass_disabled: bool,
    pub(super) render_target: Option<RenderTargetId>,
    pub(super) render_target_extent: Extent2D,
    pub(super) last_cursor_state: CursorState,
}

/// Runtime profile state resolved from declarative host/plugin config.
pub(super) struct RenderRuntimeProfileState {
    pub(super) profile: RenderRuntimeProfile,
    applied_hardware_tier: Option<RenderHardwareTier>,
}

impl RenderRuntimeProfileState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            profile: RenderRuntimeProfile::load(),
            applied_hardware_tier: None,
        }
    }

    pub(super) fn apply_hardware_tier_once(&mut self, tier: RenderHardwareTier) {
        if self.applied_hardware_tier == Some(tier) || tier == RenderHardwareTier::Unknown {
            return;
        }
        if !self.profile.accepts_hardware_tier_resolution() {
            log::info!(
                "render runtime profile: startup profile '{}' is explicit; hardware_tier={:?} will not override user-selected config",
                self.profile.id,
                tier,
            );
            self.applied_hardware_tier = Some(tier);
            return;
        }
        self.profile.apply_hardware_tier(tier);
        self.applied_hardware_tier = Some(tier);
        log::info!(
            "render runtime profile: resolved hardware_tier={:?} effective_profile='{}' gpu_safe={} shadows={} hdr={} postfx={} deferred={} terrain_streaming={}",
            tier,
            self.profile.id,
            self.profile.gpu_safe_enabled(),
            self.profile.shadows_enabled(),
            self.profile.hdr_scene_enabled(),
            self.profile.postfx_enabled(),
            self.profile.deferred_enabled(),
            self.profile.use_runtime_terrain_streaming(),
        );
    }
}


impl RenderViewportState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            clear_color: [0.0, 0.0, 0.0, 1.0],
            last_w: 0,
            last_h: 0,
            last_vp_w: 0,
            last_vp_h: 0,
            last_aspect: 1.0,
            pass_disabled: false,
            render_target: None,
            render_target_extent: Extent2D::new(0, 0),
            last_cursor_state: CursorState::released(),
        }
    }
}

/// Shadow cache and refresh state. Kept separate from graph/backend adapter state
/// so future shadow providers can be replaced without touching Vulkan.
pub(super) struct RenderShadowRuntimeState {
    pub(super) render_target: Option<RenderTargetId>,
    pub(super) render_target_resolution: u32,
    pub(super) cache_valid: bool,
    pub(super) last_refresh_frame: u64,
    pub(super) refresh_period_frames: u64,
    pub(super) warmup_defer_frames_remaining: u8,
    pub(super) current_caster_cull: Option<super::module_impl::shadows::ShadowCasterCull>,
    pub(super) cached_shadow_frame: Option<newengine_render_feature_api::ShadowFrame>,
    pub(super) unsupported_point_warning_emitted: bool,
    pub(super) unsupported_spot_warning_emitted: bool,
}

impl RenderShadowRuntimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            render_target: None,
            render_target_resolution: 0,
            cache_valid: false,
            last_refresh_frame: 0,
            refresh_period_frames: super::render_quality::shadow_refresh_period_frames(),
            warmup_defer_frames_remaining: super::render_quality::SHADOW_WARMUP_DEFER_FRAMES,
            current_caster_cull: None,
            cached_shadow_frame: None,
            unsupported_point_warning_emitted: false,
            unsupported_spot_warning_emitted: false,
        }
    }
}


/// Engine.jobs-backed CPU decode job for a material texture request.
///
/// The render controller owns only the ticket/result bridge. The actual heavy
/// work runs on the engine-owned job system, not on ad-hoc per-request threads.
pub(super) struct MaterialTextureDecodeJob {
    pub(super) ticket: JobTicket,
    pub(super) result: Arc<Mutex<Option<AssetResult<RuntimeTextureAsset>>>>,
}

impl MaterialTextureDecodeJob {
    #[inline]
    pub(super) fn is_complete(&self) -> bool {
        self.ticket.is_complete()
    }

    #[inline]
    pub(super) fn take_result(&self) -> Option<AssetResult<RuntimeTextureAsset>> {
        self.result.lock().take()
    }
}

/// Material-domain GPU state owned by the render controller.
///
/// This state is deliberately separate from mesh caches and transient lifetime
/// queues so material providers can evolve without turning the controller state
/// into a god-object again.
pub(super) struct RenderMaterialGpuState {
    pub(super) registry: MaterialGpuRegistry,
    pub(super) primary_lit_pipeline_key: Option<MaterialGpuPipelineKey>,
    pub(super) textures: FxHashMap<String, MaterialTextureGpuResidency>,
    pub(super) texture_queue: VecDeque<String>,
    pub(super) texture_decode_jobs: FxHashMap<String, MaterialTextureDecodeJob>,
    pub(super) per_draw_ubo: FxHashMap<u64, PerDrawUbo>,
}

impl RenderMaterialGpuState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            registry: MaterialGpuRegistry::default(),
            primary_lit_pipeline_key: None,
            textures: FxHashMap::default(),
            texture_queue: VecDeque::new(),
            texture_decode_jobs: FxHashMap::default(),
            per_draw_ubo: FxHashMap::default(),
        }
    }
}

/// Mesh and debug-geometry GPU caches.
pub(super) struct RenderMeshGpuState {
    pub(super) prim_cache: PrimGpuCache,
    pub(super) terrain_cache: TerrainGpuCache,
    pub(super) instance_uploader: InstanceBufferUploader,
    pub(super) collision_lines: Option<DebugLineGpu>,
}

impl RenderMeshGpuState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            prim_cache: PrimGpuCache::default(),
            terrain_cache: TerrainGpuCache::default(),
            instance_uploader: InstanceBufferUploader::default(),
            collision_lines: None,
        }
    }
}

/// Deferred native resource destruction queues.
pub(super) struct RenderGpuLifetimeState {
    pub(super) resources: RenderGpuLifetimeQueue,
}

impl RenderGpuLifetimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            resources: RenderGpuLifetimeQueue::new(),
        }
    }
}

/// GPU-side render controller state grouped by domain.
///
/// The reusable controller owns orchestration caches only. Renderer-native
/// objects stay behind RenderApi/render.api, while profile-owned feature packs
/// provide material, mesh and light extraction behavior explicitly.
pub(super) struct RenderGpuSceneState {
    pub(super) material: RenderMaterialGpuState,
    pub(super) meshes: RenderMeshGpuState,
    pub(super) lifetimes: RenderGpuLifetimeState,
}

impl RenderGpuSceneState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            material: RenderMaterialGpuState::new(),
            meshes: RenderMeshGpuState::new(),
            lifetimes: RenderGpuLifetimeState::new(),
        }
    }

    #[inline]
    pub(super) fn material_pipeline_profile(&self) -> MaterialPipelineBuildProfile {
        self.material_pipeline_profile_for(super::render_quality::SCENE_HDR_COLOR_FORMAT)
    }

    #[inline]
    pub(super) fn material_pipeline_profile_for(
        &self,
        scene_color_format: newengine_core::render::TextureFormat,
    ) -> MaterialPipelineBuildProfile {
        MaterialPipelineBuildProfile::new(
            scene_color_format,
            super::render_quality::SHADOW_MAP_COLOR_FORMAT,
        )
    }

    pub(super) fn primary_lit_pipeline_key(
        &self,
    ) -> newengine_core::EngineResult<MaterialGpuPipelineKey> {
        self.material.primary_lit_pipeline_key.ok_or_else(|| {
            newengine_core::EngineError::other(
                "render material registry: no primary lit material domain selected",
            )
        })
    }

    #[inline]
    pub(super) fn require_material_pipeline(
        &mut self,
        key: MaterialGpuPipelineKey,
        r: &mut dyn newengine_core::render::RenderApi,
    ) -> newengine_core::EngineResult<MaterialGpuPipeline> {
        let profile = self.material_pipeline_profile();
        self.material.registry.require_pipeline(key, profile, r)
    }

    #[inline]
    pub(super) fn require_material_pipeline_for(
        &mut self,
        key: MaterialGpuPipelineKey,
        scene_color_format: newengine_core::render::TextureFormat,
        r: &mut dyn newengine_core::render::RenderApi,
    ) -> newengine_core::EngineResult<MaterialGpuPipeline> {
        let profile = self.material_pipeline_profile_for(scene_color_format);
        self.material.registry.require_pipeline(key, profile, r)
    }

    pub(super) fn require_primary_lit_pipeline(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
    ) -> newengine_core::EngineResult<LitPipeline> {
        let key = self.primary_lit_pipeline_key()?;
        self.require_material_pipeline(key, r)?.lit().ok_or_else(|| {
            newengine_core::EngineError::other(format!(
                "render material registry: primary material domain produced no lit pipeline key={:?}",
                key
            ))
        })
    }

    pub(super) fn require_primary_lit_pipeline_for(
        &mut self,
        scene_color_format: newengine_core::render::TextureFormat,
        r: &mut dyn newengine_core::render::RenderApi,
    ) -> newengine_core::EngineResult<LitPipeline> {
        let key = self.primary_lit_pipeline_key()?;
        self.require_material_pipeline_for(key, scene_color_format, r)?.lit().ok_or_else(|| {
            newengine_core::EngineError::other(format!(
                "render material registry: selected material domain is not a lit pipeline key='{}'",
                key.as_str()
            ))
        })
    }
}

/// Frame/gameplay view state required by extraction. Renderer backend adapters
/// must not access this directly; it is consumed before RenderFrameEnvelope is
/// submitted.
pub(super) struct RenderFrameRuntimeState {
    pub(super) frame_index: u64,
    pub(super) last_pick_seq: u64,
    /// Last camera frame observed by render orchestration.
    ///
    /// This is a pure DTO snapshot from the camera contract boundary. Render
    /// runtime must not own `newengine-camera` projection/controller/nav state.
    pub(super) last_camera_snapshot: Option<CameraFrameSnapshot>,
    pub(super) sim_schedule: newengine_sim::SimSchedule,
    pub(super) input_systems: crate::input_systems::InputRuntimeSystems,
    pub(super) last_play_mode: crate::GameRunMode,
}

impl RenderFrameRuntimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            frame_index: 0,
            last_pick_seq: 0,
            last_camera_snapshot: None,
            sim_schedule: crate::gameplay::default_sim_schedule(),
            input_systems: crate::input_systems::InputRuntimeSystems::new(),
            last_play_mode: crate::GameRunMode::Staging,
        }
    }
}

pub(super) struct RenderDiagnosticsRuntimeState {
    pub(super) overlay_metrics: RuntimeOverlayMetrics,
}

impl RenderDiagnosticsRuntimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            overlay_metrics: RuntimeOverlayMetrics::new(),
        }
    }
}




pub(super) struct RenderUiSurfaceRuntimeState {
    pub(super) primary: super::module_impl::ui_node_surface::RenderUiNodeSurfaceState,
}

impl RenderUiSurfaceRuntimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            primary: super::module_impl::ui_node_surface::RenderUiNodeSurfaceState::new(),
        }
    }
}

