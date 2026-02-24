#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{Perspective, Projection};
use newengine_core::render::Extent2D;
use newengine_math::collections::FxHashMap;
use newengine_math::Vec3;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::LIT_UBO_SIZE;
use super::gpu::{GridGpu, LitPipeline, PrimitiveGpu};

type PrimGpuCache = FxHashMap<newengine_primitives::PrimitiveId, PrimitiveGpu>;

#[derive(Clone, Copy)]
pub(super) struct PerDrawUbo {
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub last_seen_frame: u64,
}

pub struct EditorRenderController {
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

    pub(super) viewport_rt: Option<newengine_core::render::RenderTargetId>,
    pub(super) viewport_rt_extent: Extent2D,

    /// Render targets must be destroyed only after in-flight command buffers are done.
    /// We approximate this with a small frame delay (triple-buffer friendly).
    pub(super) deferred_rts: Vec<(newengine_core::render::RenderTargetId, u64)>,

    pub(super) grid: Option<GridGpu>,
    pub(super) lit: Option<LitPipeline>,
    pub(super) prim_cache: PrimGpuCache,

    /// Per-entity/per-draw uniform buffers to avoid "last write wins" hazards.
    ///
    /// Many backends (notably Vulkan) record draw commands first and execute them later.
    /// If we keep a single UBO and overwrite it between draws, all draws can observe the
    /// final UBO contents, making it look like objects replace each other.
    pub(super) per_draw_ubo: FxHashMap<u64, PerDrawUbo>,

    pub(super) frame_index: u64,

    // Camera framing:
    // - frame_once on startup/aspect change
    // - expand frame only when scene grows (never shrink on spawn)
    pub(super) framed_once: bool,
    pub(super) framed_radius: f32,

    pub(super) last_bounds_center: Vec3,
    pub(super) last_bounds_radius: f32,

    pub(super) last_pick_seq: u64,

    /// UI-triggered explicit "frame scene" requests.
    pub(super) last_frame_seq: u64,
}

impl EditorRenderController {
    #[inline]
    pub fn new(
        clear_color: [f32; 4],
        viewport_bridge: std::sync::Arc<ViewportBridge>,
        plugins_bridge: std::sync::Arc<PluginManagerBridge>,
        scene_bridge: std::sync::Arc<SceneBridge>,
        previews: std::sync::Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
    ) -> Self {
        // Camera controller state lives in ECS (EditorCameraController + CameraRigComp).

        let projection =
            Projection::Perspective(Perspective::new(60.0f32.to_radians(), 1.0, 0.01, 1000.0));

        Self {
            clear_color,
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

            viewport_rt: None,
            viewport_rt_extent: Extent2D::new(0, 0),

            deferred_rts: Vec::new(),

            grid: None,
            lit: None,
            prim_cache: PrimGpuCache::default(),

            per_draw_ubo: FxHashMap::default(),

            frame_index: 0,

            framed_once: false,
            framed_radius: 0.0,

            last_bounds_center: Vec3::ZERO,
            last_bounds_radius: 1.0,

            last_pick_seq: 0,

            last_frame_seq: 0,
        }
    }

    #[inline]
    pub(super) fn ensure_per_draw_ubo(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: super::gpu::LitPipeline,
        key: u64,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        if let Some(e) = self.per_draw_ubo.get(&key).copied() {
            return Ok(e);
        }

        let ubo = r.create_buffer(
            newengine_core::render::BufferDesc::new(
                LIT_UBO_SIZE,
                newengine_core::render::BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
                .with_label("editor_lit_entity_ubo"),
        )?;

        let bg = r.create_bind_group(
            newengine_core::render::BindGroupDesc::new(lit.bgl)
                .with_label("editor_lit_entity_bg")
                .with_uniform0(newengine_core::render::BufferBinding::new(
                    ubo,
                    0,
                    LIT_UBO_SIZE,
                )),
        )?;

        let entry = PerDrawUbo {
            ubo,
            bg,
            last_seen_frame: self.frame_index,
        };
        self.per_draw_ubo.insert(key, entry);
        Ok(entry)
    }

    pub(super) fn gc_per_draw_ubos(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        let now = self.frame_index;
        let grace = 2_u64;

        let mut dead: Vec<u64> = Vec::new();
        for (k, v) in &self.per_draw_ubo {
            if now.saturating_sub(v.last_seen_frame) > grace {
                dead.push(*k);
            }
        }
        for k in dead {
            if let Some(v) = self.per_draw_ubo.remove(&k) {
                r.destroy_bind_group(v.bg);
                r.destroy_buffer(v.ubo);
            }
        }
    }

    pub(super) fn gc_deferred_rts(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        let now = self.frame_index;
        let grace = 4_u64;

        let mut i = 0;
        while i < self.deferred_rts.len() {
            let (rt, born) = self.deferred_rts[i];
            if now.saturating_sub(born) > grace {
                r.destroy_render_target(rt);
                self.deferred_rts.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}
