#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{Perspective, Projection};
use newengine_core::host_events::CursorState;
use newengine_core::render::{Extent2D, RenderTargetId, SamplerId, TextureDesc, TextureFormat, TextureId, TextureUsage};
use newengine_math::collections::{FxHashMap, FxHashSet};

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::LIT_UBO_SIZE;
use super::gpu::{load_rgba_texture_asset, DebugLineGpu, GridGpu, LitPipeline, PrimitiveGpu};

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

    pub(super) previews_disabled: bool,
    pub(super) viewport_pass_disabled: bool,

    pub(super) viewport_rt: Option<newengine_core::render::RenderTargetId>,
    pub(super) viewport_rt_extent: Extent2D,
    pub(super) shadow_rt: Option<RenderTargetId>,
    pub(super) shadow_rt_resolution: u32,
    pub(super) deferred_rts: Vec<(newengine_core::render::RenderTargetId, u64)>,

    pub(super) grid: Option<GridGpu>,
    pub(super) lit: Option<LitPipeline>,
    pub(super) prim_cache: PrimGpuCache,
    pub(super) terrain_cache: TerrainGpuCache,
    pub(super) material_texture_cache: FxHashMap<String, TextureId>,
    pub(super) material_texture_failures: FxHashSet<String>,
    pub(super) per_draw_ubo: FxHashMap<u64, PerDrawUbo>,

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

impl EditorRenderController {
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
            deferred_rts: Vec::new(),
            grid: None,
            lit: None,
            prim_cache: PrimGpuCache::default(),
            terrain_cache: TerrainGpuCache::default(),
            material_texture_cache: FxHashMap::default(),
            material_texture_failures: FxHashSet::default(),
            per_draw_ubo: FxHashMap::default(),
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

    pub(super) fn ensure_material_texture(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: &str,
    ) -> newengine_core::EngineResult<TextureId> {
        if let Some(id) = self.material_texture_cache.get(path).copied() {
            return Ok(id);
        }
        let (extent, rgba) = load_rgba_texture_asset(path)?;
        let tex = r.create_texture(
            TextureDesc::new(extent, TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
                .with_label(format!("material_tex:{path}"))
                .with_data(rgba),
        )?;
        self.material_texture_cache.insert(path.to_string(), tex);
        Ok(tex)
    }


    #[inline]
    pub(super) fn material_texture_or_default(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: Option<&str>,
        fallback: TextureId,
    ) -> TextureId {
        let Some(path) = path else {
            return fallback;
        };

        match self.ensure_material_texture(r, path) {
            Ok(texture) => texture,
            Err(e) => {
                if self.material_texture_failures.insert(path.to_string()) {
                    log::warn!(
                        "render controller: material texture disabled path='{}' err='{}'",
                        path,
                        e
                    );
                }
                fallback
            }
        }
    }

    pub(super) fn ensure_per_draw_ubo(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: super::gpu::LitPipeline,
        key: u64,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        self.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            lit.white_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
        )
    }

    pub(super) fn ensure_per_draw_ubo_with_binding(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: super::gpu::LitPipeline,
        key: u64,
        base_texture: TextureId,
        normal_texture: TextureId,
        roughness_texture: TextureId,
        shadow_texture: TextureId,
        sampler: SamplerId,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        if let Some(mut e) = self.per_draw_ubo.get(&key).copied() {
            if e.base_texture == base_texture
                && e.normal_texture == normal_texture
                && e.roughness_texture == roughness_texture
                && e.shadow_texture == shadow_texture
                && e.sampler == sampler
            {
                return Ok(e);
            }
            r.destroy_bind_group(e.bg);
            let bg = r.create_bind_group(
                newengine_core::render::BindGroupDesc::new(lit.bgl)
                    .with_label("editor_lit_entity_bg")
                    .with_uniform0(newengine_core::render::BufferBinding::new(e.ubo, 0, LIT_UBO_SIZE))
                    .with_texture0(base_texture)
                    .with_texture1(normal_texture)
                    .with_texture2(roughness_texture)
                    .with_texture3(shadow_texture)
                    .with_sampler0(sampler),
            )?;
            e.bg = bg;
            e.base_texture = base_texture;
            e.normal_texture = normal_texture;
            e.roughness_texture = roughness_texture;
            e.shadow_texture = shadow_texture;
            e.sampler = sampler;
            self.per_draw_ubo.insert(key, e);
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
                .with_uniform0(newengine_core::render::BufferBinding::new(ubo, 0, LIT_UBO_SIZE))
                .with_texture0(base_texture)
                .with_texture1(normal_texture)
                .with_texture2(roughness_texture)
                .with_texture3(shadow_texture)
                .with_sampler0(sampler),
        )?;

        let entry = PerDrawUbo {
            ubo,
            bg,
            base_texture,
            normal_texture,
            roughness_texture,
            shadow_texture,
            sampler,
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
