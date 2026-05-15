#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{Perspective, Projection};
use newengine_core::host_events::CursorState;
use newengine_core::render::{
    Extent2D, RenderTargetId, SamplerId, TextureId,
};
use newengine_math::collections::FxHashMap;
use std::collections::VecDeque;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::{DebugLineGpu, GridGpu, LitPipeline, PrimitiveGpu};
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
pub(super) struct PerDrawUbo {
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub base_texture: TextureId,
    pub normal_texture: TextureId,
    pub roughness_texture: TextureId,
    pub shadow_texture: TextureId,
    pub sampler: SamplerId,
    pub last_seen_frame: u64,
}

pub struct RuntimeRenderController {
    pub(super) clear_color: [f32; 4],
    pub(super) last_w: u32,
    pub(super) last_h: u32,

    pub(super) last_vp_w: u32,
    pub(super) last_vp_h: u32,
    pub(super) last_aspect: f32,
    pub(super) projection: Projection,

    pub(super) viewport_bridge: std::sync::Arc<ViewportBridge>,
    pub(super) plugins_bridge: std::sync::Arc<PluginManagerBridge>,
    pub(super) scene_bridge: std::sync::Arc<SceneBridge>,

    pub(super) previews:
        std::sync::Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,

    pub(super) previews_disabled: bool,
    pub(super) viewport_pass_disabled: bool,

    pub(super) viewport_rt: Option<newengine_core::render::RenderTargetId>,
    pub(super) viewport_rt_extent: Extent2D,
    pub(super) shadow_rt: Option<RenderTargetId>,
    pub(super) shadow_rt_resolution: u32,
    pub(super) shadow_cache_valid: bool,
    pub(super) shadow_last_refresh_frame: u64,
    pub(super) shadow_refresh_period_frames: u64,
    pub(super) shadow_warmup_defer_frames_remaining: u8,
    pub(super) unsupported_point_shadow_warning_emitted: bool,
    pub(super) unsupported_spot_shadow_warning_emitted: bool,
    pub(super) render_target_lifetimes: RenderTargetLifetimeQueue,

    pub(super) grid: Option<GridGpu>,
    pub(super) lit: Option<LitPipeline>,
    pub(super) prim_cache: PrimGpuCache,
    pub(super) terrain_cache: TerrainGpuCache,
    pub(super) material_textures: FxHashMap<String, MaterialTextureGpuResidency>,
    pub(super) material_texture_queue: VecDeque<String>,
    pub(super) per_draw_ubo: FxHashMap<u64, PerDrawUbo>,
    pub(super) instance_uploader: InstanceBufferUploader,
    pub(super) overlay_metrics: RuntimeOverlayMetrics,

    pub(super) frame_index: u64,
    pub(super) last_pick_seq: u64,

    pub(super) collision_lines: Option<DebugLineGpu>,
    pub(super) sim_schedule: newengine_sim::SimSchedule,
    pub(super) last_play_mode: crate::EditorPlayMode,
    pub(super) camera_nav: newengine_camera_runtime::CameraNavState,
    pub(super) play_session: Option<PlaySessionSnapshot>,
    pub(super) runtime_session: Option<crate::gameplay::RuntimeWorldSnapshot>,
    pub(super) last_cursor_state: CursorState,
}

impl RuntimeRenderController {
    #[inline]
    pub fn new(
        viewport_bridge: std::sync::Arc<ViewportBridge>,
        plugins_bridge: std::sync::Arc<PluginManagerBridge>,
        scene_bridge: std::sync::Arc<SceneBridge>,
        previews: std::sync::Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
    ) -> Self {
        let projection =
            Projection::Perspective(Perspective::new(60.0f32.to_radians(), 1.0, 0.01, 1000.0));

        Self {
            clear_color: [0.0, 0.0, 0.0, 1.0],
            last_w: 0,
            last_h: 0,
            last_vp_w: 0,
            last_vp_h: 0,
            last_aspect: 1.0,
            projection,
            viewport_bridge,
            plugins_bridge,
            scene_bridge,
            previews,
            previews_disabled: false,
            viewport_pass_disabled: false,
            viewport_rt: None,
            viewport_rt_extent: Extent2D::new(0, 0),
            shadow_rt: None,
            shadow_rt_resolution: 0,
            shadow_cache_valid: false,
            shadow_last_refresh_frame: 0,
            shadow_refresh_period_frames: 90,
            shadow_warmup_defer_frames_remaining: super::render_quality::SHADOW_WARMUP_DEFER_FRAMES,
            unsupported_point_shadow_warning_emitted: false,
            unsupported_spot_shadow_warning_emitted: false,
            render_target_lifetimes: RenderTargetLifetimeQueue::new(),
            grid: None,
            lit: None,
            prim_cache: PrimGpuCache::default(),
            terrain_cache: TerrainGpuCache::default(),
            material_textures: FxHashMap::default(),
            material_texture_queue: VecDeque::new(),
            per_draw_ubo: FxHashMap::default(),
            instance_uploader: InstanceBufferUploader::default(),
            overlay_metrics: RuntimeOverlayMetrics::new(),
            frame_index: 0,
            last_pick_seq: 0,
            collision_lines: None,
            sim_schedule: crate::gameplay::default_sim_schedule(),
            last_play_mode: crate::EditorPlayMode::Edit,
            camera_nav: newengine_camera_runtime::CameraNavState::default(),
            play_session: None,
            runtime_session: None,
            last_cursor_state: CursorState::released(),
        }
    }
}
