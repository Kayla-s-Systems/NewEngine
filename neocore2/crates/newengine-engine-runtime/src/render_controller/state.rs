#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{Perspective, Projection};
use newengine_core::host_events::CursorState;
use newengine_core::render::{Extent2D, RenderTargetId, SamplerId, TextureId};
use newengine_math::collections::FxHashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::{DebugLineGpu, LitPipeline, PrimitiveGpu};
use super::material_bindings::MaterialTextureGpuResidency;
use super::metrics::RuntimeOverlayMetrics;
use super::module_impl::instancing::InstanceBufferUploader;
use super::resource_lifetime::RenderTargetLifetimeQueue;

type PrimGpuCache = FxHashMap<newengine_primitives::PrimitiveId, PrimitiveGpu>;
type TerrainGpuCache = FxHashMap<u64, PrimitiveGpu>;

#[derive(Clone, Copy, Debug)]
pub(super) struct PlaySessionSnapshot {
    pub(super) cam_id: newengine_ecs::EntityId,
    pub(super) rig: newengine_sim::CameraRigComp,
    pub(super) transform: Option<newengine_transform::Transform>,
}

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
    pub(super) projection: Projection,
    pub(super) pass_disabled: bool,
    pub(super) render_target: Option<RenderTargetId>,
    pub(super) render_target_extent: Extent2D,
    pub(super) last_cursor_state: CursorState,
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
            projection: Projection::Perspective(Perspective::new(
                60.0f32.to_radians(),
                1.0,
                0.01,
                1000.0,
            )),
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
            refresh_period_frames: 90,
            warmup_defer_frames_remaining: super::render_quality::SHADOW_WARMUP_DEFER_FRAMES,
            unsupported_point_warning_emitted: false,
            unsupported_spot_warning_emitted: false,
        }
    }
}

/// GPU resource registries and per-frame resource lifetime queues.
///
/// These resources are still manipulated through RenderApi, but they are no
/// longer mixed with gameplay/camera/session state in the controller layout.
pub(super) struct RenderGpuSceneState {
    pub(super) lit: Option<LitPipeline>,
    pub(super) prim_cache: PrimGpuCache,
    pub(super) terrain_cache: TerrainGpuCache,
    pub(super) material_textures: FxHashMap<String, MaterialTextureGpuResidency>,
    pub(super) material_texture_queue: VecDeque<String>,
    pub(super) per_draw_ubo: FxHashMap<u64, PerDrawUbo>,
    pub(super) instance_uploader: InstanceBufferUploader,
    pub(super) collision_lines: Option<DebugLineGpu>,
    pub(super) render_target_lifetimes: RenderTargetLifetimeQueue,
}

impl RenderGpuSceneState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            lit: None,
            prim_cache: PrimGpuCache::default(),
            terrain_cache: TerrainGpuCache::default(),
            material_textures: FxHashMap::default(),
            material_texture_queue: VecDeque::new(),
            per_draw_ubo: FxHashMap::default(),
            instance_uploader: InstanceBufferUploader::default(),
            collision_lines: None,
            render_target_lifetimes: RenderTargetLifetimeQueue::new(),
        }
    }
}

/// Frame/gameplay view state required by extraction. Renderer backend adapters
/// must not access this directly; it is consumed before RenderFrameEnvelope is
/// submitted.
pub(super) struct RenderFrameRuntimeState {
    pub(super) frame_index: u64,
    pub(super) last_pick_seq: u64,
    pub(super) sim_schedule: newengine_sim::SimSchedule,
    pub(super) last_play_mode: crate::GameRunMode,
    pub(super) camera_nav: newengine_camera_runtime::CameraNavState,
    pub(super) play_session: Option<PlaySessionSnapshot>,
    pub(super) runtime_session: Option<crate::gameplay::RuntimeWorldSnapshot>,
}

impl RenderFrameRuntimeState {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            frame_index: 0,
            last_pick_seq: 0,
            sim_schedule: crate::gameplay::default_sim_schedule(),
            last_play_mode: crate::GameRunMode::Staging,
            camera_nav: newengine_camera_runtime::CameraNavState::default(),
            play_session: None,
            runtime_session: None,
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

