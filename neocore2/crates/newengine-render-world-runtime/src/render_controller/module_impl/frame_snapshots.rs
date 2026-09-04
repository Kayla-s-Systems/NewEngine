#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::Bounds;
use newengine_core::render::{BindGroupId, Extent2D, PipelineId, TextureId};
use newengine_materials::MaterialRef;
use newengine_math::{Mat4, Vec3};
use newengine_model_domain_api::{FoliageInstanceRuntime, MeshRenderOptions};
use newengine_primitives::Primitive;
use newengine_render_feature_api::BoundsSnap;
use newengine_scene::Scene;
use newengine_transform::GlobalTransform;
use std::sync::Arc;

use newengine_gameplay_world_runtime::gameplay::{
    display_shadow_caster_visible_in_mode, display_visible_in_mode, player_render_model_matrix,
    EnvironmentDomeRenderState, PlayerSkinBinding, PlayerVisualKind, PlayerVisualPart,
    WorldItemPresentation, WorldItemVisualPart,
};

use crate::render_controller::gpu::{PlayerSkinGpu, PrimitiveGpu};

use super::{scene, RuntimeRenderController};

/// Immutable primitive input captured once for all render passes in a frame.
///
/// The snapshot owns only CPU-side domain values. It keeps ECS access and gameplay
/// presentation rules outside backend recording while giving every pass a coherent
/// view of the project scene.
pub(in crate::render_controller) struct PrimitiveSceneSnapshot {
    frame_index: u64,
    scene_key: usize,
    runtime: bool,
    pub(super) queried_count: usize,
    pub(super) entries: Box<[PrimitiveSceneEntry]>,
}

pub(super) struct PrimitiveSceneEntry {
    pub(super) entity_key: u64,
    pub(super) primitive: Primitive,
    pub(super) render_model: Mat4,
    pub(super) material_ref: Option<MaterialRef>,
    pub(super) render_options: MeshRenderOptions,
    pub(super) foliage_runtime: Option<FoliageInstanceRuntime>,
    pub(super) environment_dome: Option<EnvironmentDomeRenderState>,
    pub(super) local_bounds: Option<(Vec3, f32)>,
    pub(super) authored_pbr_required: bool,
}

impl PrimitiveSceneSnapshot {
    fn capture(frame_index: u64, scene: &Scene, runtime: bool) -> Self {
        let world = scene.world();
        let mut queried_count = 0usize;
        let mut entries = Vec::new();

        for (id, primitive, transform) in world.query2::<Primitive, GlobalTransform>() {
            queried_count = queried_count.saturating_add(1);
            if world.get::<PlayerSkinBinding>(id).is_some()
                || !display_visible_in_mode(world, id, runtime)
            {
                continue;
            }

            let render_model = player_render_model_matrix(world, id, transform.0);
            let render_options = world
                .get::<MeshRenderOptions>(id)
                .cloned()
                .unwrap_or_else(MeshRenderOptions::world_opaque);
            let authored_pbr_required = world
                .get::<PlayerVisualPart>(id)
                .is_some_and(|part| part.kind == PlayerVisualKind::EquippedWeapon)
                || world
                    .get::<WorldItemVisualPart>(id)
                    .and_then(|part| world.get::<WorldItemPresentation>(part.owner))
                    .and_then(|presentation| presentation.model_ref.as_deref())
                    .is_some_and(|model_ref| !model_ref.trim().is_empty());
            let local_bounds = world
                .get::<Bounds>(id)
                .map(|bounds| (bounds.local_sphere.center, bounds.local_sphere.radius));

            entries.push(PrimitiveSceneEntry {
                entity_key: id.stable_u64(),
                primitive: *primitive,
                render_model,
                material_ref: world.get::<MaterialRef>(id).copied(),
                render_options,
                foliage_runtime: world.get::<FoliageInstanceRuntime>(id).copied(),
                environment_dome: world.get::<EnvironmentDomeRenderState>(id).cloned(),
                local_bounds,
                authored_pbr_required,
            });
        }

        Self {
            frame_index,
            scene_key: scene as *const Scene as usize,
            runtime,
            queried_count,
            entries: entries.into_boxed_slice(),
        }
    }

    #[inline]
    fn matches(&self, frame_index: u64, scene: &Scene, runtime: bool) -> bool {
        self.frame_index == frame_index
            && self.scene_key == scene as *const Scene as usize
            && self.runtime == runtime
    }
}

/// Frame-coherent admission set for skinned directional-shadow casters.
///
/// CSM cascades share entity/material/owner admission; only the light matrix and
/// projected-size decision vary per cascade. Capturing this once prevents four
/// complete ECS scans of the same character parts every frame.
pub(in crate::render_controller) struct SkinnedShadowSceneSnapshot {
    frame_index: u64,
    scene_key: usize,
    runtime: bool,
    pub(super) entries: Box<[SkinnedShadowSceneEntry]>,
}

pub(super) struct SkinnedShadowSceneEntry {
    pub(super) entity: newengine_ecs::EntityId,
    pub(super) owner: newengine_ecs::EntityId,
    pub(super) primitive: Primitive,
    pub(super) render_model: Mat4,
    pub(super) material_ref: Option<MaterialRef>,
    pub(super) proxy_center_ws: Vec3,
    pub(super) proxy_radius_ws: f32,
    pub(super) pose_generation: u64,
}

/// GPU/material state resolved once per frame for all skinned shadow views.
///
/// The renderer remains the owner of every native resource. This immutable plan stores only
/// frame-local handles and draw constants so directional cascades do not repeat ECS/material/GPU
/// cache resolution. Cascade-specific visibility and light matrices intentionally remain outside.
pub(in crate::render_controller) struct PreparedSkinnedShadowFramePlan {
    pub(super) frame_index: u64,
    pub(super) scene_key: usize,
    pub(super) runtime: bool,
    pub(super) entries: Box<[PreparedSkinnedShadowCaster]>,
}

#[derive(Clone, Copy)]
pub(super) struct PreparedSkinnedShadowCaster {
    pub(super) entity: newengine_ecs::EntityId,
    pub(super) primitive: Primitive,
    pub(super) render_model: Mat4,
    pub(super) proxy_center_ws: Vec3,
    pub(super) proxy_radius_ws: f32,
    pub(super) primitive_gpu: PrimitiveGpu,
    pub(super) skin_gpu: PlayerSkinGpu,
    pub(super) palette_bg: BindGroupId,
    pub(super) base_texture: TextureId,
    pub(super) pipeline: PipelineId,
    pub(super) alpha_cutoff: f32,
    pub(super) uv_transform: [f32; 4],
}

impl PreparedSkinnedShadowFramePlan {
    #[inline]
    pub(super) fn matches(&self, frame_index: u64, scene: &Scene, runtime: bool) -> bool {
        self.frame_index == frame_index
            && self.scene_key == scene as *const Scene as usize
            && self.runtime == runtime
    }
}

impl SkinnedShadowSceneSnapshot {
    fn capture(frame_index: u64, scene: &Scene, runtime: bool) -> Self {
        let world = scene.world();
        let mut entries = Vec::new();
        for (entity, primitive, global) in world.query2::<Primitive, GlobalTransform>() {
            let Some(skin) = world.get::<PlayerSkinBinding>(entity) else {
                continue;
            };
            if !display_shadow_caster_visible_in_mode(world, entity, runtime) {
                continue;
            }
            let render_model = player_render_model_matrix(world, entity, global.0);
            let owner_height = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerModelBinding>(skin.owner)
                .map(|binding| binding.target_height.max(1.0))
                .unwrap_or(2.0);
            let proxy_center_ws = world
                .get::<GlobalTransform>(skin.owner)
                .map(|owner_global| {
                    owner_global
                        .0
                        .transform_point3(Vec3::new(0.0, owner_height * 0.5, 0.0))
                })
                .unwrap_or_else(|| render_model.transform_point3(Vec3::ZERO));
            let pose_generation = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerModelBinding>(skin.owner)
                .map(|binding| binding.assignment_revision)
                .unwrap_or(0);
            entries.push(SkinnedShadowSceneEntry {
                entity,
                owner: skin.owner,
                primitive: *primitive,
                render_model,
                material_ref: world.get::<MaterialRef>(entity).copied(),
                proxy_center_ws,
                proxy_radius_ws: owner_height * 0.80 + 0.45,
                pose_generation,
            });
        }
        Self {
            frame_index,
            scene_key: scene as *const Scene as usize,
            runtime,
            entries: entries.into_boxed_slice(),
        }
    }

    #[inline]
    fn matches(&self, frame_index: u64, scene: &Scene, runtime: bool) -> bool {
        self.frame_index == frame_index
            && self.scene_key == scene as *const Scene as usize
            && self.runtime == runtime
    }
}

impl RuntimeRenderController {
    /// Returns the frame-coherent primitive snapshot and whether it was already captured.
    pub(super) fn primitive_scene_snapshot(
        &mut self,
        scene: &Scene,
        runtime: bool,
    ) -> (Arc<PrimitiveSceneSnapshot>, bool) {
        let frame_index = self.frame.frame_index;
        if let Some(snapshot) = self.frame.primitive_scene_snapshot.as_ref() {
            if snapshot.matches(frame_index, scene, runtime) {
                return (Arc::clone(snapshot), true);
            }
        }

        let snapshot = Arc::new(PrimitiveSceneSnapshot::capture(frame_index, scene, runtime));
        self.frame.primitive_scene_snapshot = Some(Arc::clone(&snapshot));
        (snapshot, false)
    }

    /// Returns skinned shadow admission captured once for all CSM cascades.
    pub(super) fn skinned_shadow_scene_snapshot(
        &mut self,
        scene: &Scene,
        runtime: bool,
    ) -> (Arc<SkinnedShadowSceneSnapshot>, bool) {
        let frame_index = self.frame.frame_index;
        if let Some(snapshot) = self.frame.skinned_shadow_scene_snapshot.as_ref() {
            if snapshot.matches(frame_index, scene, runtime) {
                return (Arc::clone(snapshot), true);
            }
        }
        let snapshot = Arc::new(SkinnedShadowSceneSnapshot::capture(
            frame_index,
            scene,
            runtime,
        ));
        self.frame.skinned_shadow_scene_snapshot = Some(Arc::clone(&snapshot));
        (snapshot, false)
    }
}

/// CPU-side scene render snapshot captured before RenderPrep/submit.
///
/// This is the first structural boundary for moving provider-safe extraction out
/// of `render.controller`. It intentionally contains DTO-like values, not
/// `RenderApi`, backend handles or mutable world references. Heavy consumers can
/// later receive this through `engine.threading` RenderPrep batches and return frame
/// packets for render-thread recording.
#[derive(Clone, Copy, Debug)]
pub(super) struct SceneRenderSnapshot {
    pub frame_index: u64,
    pub bounds: BoundsSnap,
    pub camera_position: Vec3,
    pub camera_forward: Vec3,
    pub viewport_extent: Extent2D,
    pub surface_extent: Extent2D,
    pub ui_present: bool,
    pub plugin_snapshot_present: bool,
}

impl SceneRenderSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture(
        frame_index: u64,
        scene: &Scene,
        _viewproj: Mat4,
        camera_position: Vec3,
        camera_forward: Vec3,
        viewport_extent: Extent2D,
        surface_extent: Extent2D,
        ui_present: bool,
        plugin_snapshot_present: bool,
    ) -> Self {
        Self {
            frame_index,
            bounds: scene::scene_bounds(scene).unwrap_or_else(scene::default_bounds),
            camera_position,
            camera_forward,
            viewport_extent,
            surface_extent,
            ui_present,
            plugin_snapshot_present,
        }
    }

    pub(super) fn diagnostic_detail(&self) -> String {
        format!(
            "SceneRenderSnapshot frame={} bounds_radius={:.3} viewport={}x{} surface={}x{} ui_present={} plugin_snapshot={}",
            self.frame_index,
            self.bounds.radius,
            self.viewport_extent.width,
            self.viewport_extent.height,
            self.surface_extent.width,
            self.surface_extent.height,
            self.ui_present,
            self.plugin_snapshot_present,
        )
    }
}
