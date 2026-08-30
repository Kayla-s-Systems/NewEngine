#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::Bounds;
use newengine_core::render::Extent2D;
use newengine_materials::MaterialRef;
use newengine_model_domain_api::{FoliageInstanceRuntime, MeshRenderOptions};
use newengine_primitives::Primitive;
use newengine_math::{Mat4, Vec3};
use newengine_render_feature_api::BoundsSnap;
use newengine_scene::Scene;
use newengine_transform::GlobalTransform;
use std::sync::Arc;

use crate::gameplay::{
    display_visible_in_mode, player_render_model_matrix, EnvironmentDomeRenderState,
    PlayerSkinBinding, PlayerVisualKind, PlayerVisualPart, WorldItemPresentation,
    WorldItemVisualPart,
};

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
