use std::collections::BTreeSet;

use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_model_domain_api::{
    MeshCullPolicy, MeshDepthPolicy, MeshRenderOptions, MeshRenderRole, MeshShadowPolicy,
    MeshSortPolicy, MeshTransformPolicy, MeshVisibilityPolicy,
};
use newengine_primitives::{builtins, Primitive, PrimitiveId};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::{GlobalTransform, Transform};
use newengine_ui_api::{UiEditorTransformMode, UiEditorTransformSpace};

use crate::gameplay::{DisplayMode, DisplayVisibility};
use crate::scene_bridge::{
    apply_primitive_instance, ensure_primitive_base, primitive_bounds, SceneBridge,
};

use super::{
    EditorGizmoAxis, EditorGizmoAxisComponent, EditorGizmoHandle, EditorGizmoPlane,
    EditorViewportController,
};

impl EditorViewportController {
    pub(crate) fn sync_gizmo_geometry(
        &mut self,
        scene_bridge: &SceneBridge,
        scene: &mut Scene,
        selected: Option<EntityId>,
        selection_radius: f32,
    ) {
        let visible = self.state.gizmo_visible
            && self.state.transform_mode != UiEditorTransformMode::Select
            && selected.is_some();
        if !visible {
            self.remove_gizmos(scene.world_mut());
            return;
        }
        let Some(selected) = selected else {
            return;
        };
        let Some(selected_transform) = scene.world().get::<Transform>(selected).copied() else {
            self.remove_gizmos(scene.world_mut());
            return;
        };
        let (selected_world_position, selected_world_rotation) = scene
            .world()
            .get::<GlobalTransform>(selected)
            .map(|global| {
                let (_, rotation, translation) = global.0.to_scale_rotation_translation();
                (translation, rotation)
            })
            .unwrap_or((selected_transform.position, selected_transform.rotation));
        let gizmo_orientation = match self.state.transform_space {
            UiEditorTransformSpace::World => Quat::IDENTITY,
            UiEditorTransformSpace::Local => selected_world_rotation,
        };

        let length = (selection_radius.max(0.25) * 1.8).clamp(0.65, 8.0);
        let thickness = (length * 0.045).clamp(0.025, 0.18);
        let mats_lock = scene_bridge.materials();
        let mats = mats_lock.read();
        let default_mat = mats.register_named(
            "EditorGizmo",
            newengine_materials::MaterialDescriptor::default(),
        );
        let prims_lock = scene_bridge.primitives();
        let prims = prims_lock.read();
        let world = scene.world_mut();

        let mut desired_handles = vec![
            EditorGizmoHandle::Axis(EditorGizmoAxis::X),
            EditorGizmoHandle::Axis(EditorGizmoAxis::Y),
            EditorGizmoHandle::Axis(EditorGizmoAxis::Z),
        ];
        if self.state.transform_mode == UiEditorTransformMode::Translate {
            desired_handles.extend([
                EditorGizmoHandle::Plane(EditorGizmoPlane::XY),
                EditorGizmoHandle::Plane(EditorGizmoPlane::XZ),
                EditorGizmoHandle::Plane(EditorGizmoPlane::YZ),
                EditorGizmoHandle::Center,
            ]);
        } else if self.state.transform_mode == UiEditorTransformMode::Scale {
            desired_handles.push(EditorGizmoHandle::Center);
        }

        let desired_indices = desired_handles
            .iter()
            .map(|handle| handle.index())
            .collect::<BTreeSet<_>>();
        for (index, slot) in self.gizmo_entities.iter_mut().enumerate() {
            if desired_indices.contains(&index) {
                continue;
            }
            if let Some(entity) = slot.take() {
                if world.exists(entity) {
                    let _ = world.despawn(entity);
                }
            }
        }

        for handle in desired_handles {
            let index = handle.index();
            let entity = match self.gizmo_entities[index].filter(|entity| world.exists(*entity)) {
                Some(entity) => entity,
                None => {
                    let entity = spawn_named(world, format!("__EditorGizmo{}", handle.name()));
                    self.gizmo_entities[index] = Some(entity);
                    entity
                }
            };

            let primitive = match handle {
                EditorGizmoHandle::Axis(_) => gizmo_primitive_for_mode(self.state.transform_mode),
                EditorGizmoHandle::Plane(_) | EditorGizmoHandle::Center => builtins::ID_CUBE,
            };
            let color = handle.color();
            let _ = world.insert(
                entity,
                Primitive {
                    id: primitive,
                    color,
                },
            );
            let _ = world.insert(entity, EditorGizmoAxisComponent { handle });
            let _ = world.insert(
                entity,
                DisplayVisibility {
                    mode: DisplayMode::RuntimeHidden,
                },
            );
            let _ = world.insert(entity, editor_gizmo_render_options());
            if let Some(bounds) = primitive_bounds(&prims, primitive) {
                let _ = world.insert(entity, bounds);
            }
            ensure_primitive_base(world, entity, default_mat);
            apply_primitive_instance(world, &mats, entity, default_mat, color);

            let transform = gizmo_handle_transform(
                self.state.transform_mode,
                handle,
                selected_world_position,
                gizmo_orientation,
                length,
                thickness,
            );
            let _ = world.insert(entity, transform);
        }
    }

    pub(crate) fn clear_runtime_geometry(&mut self, world: &mut World) {
        self.remove_gizmos(world);
    }

    fn remove_gizmos(&mut self, world: &mut World) {
        for entity in &mut self.gizmo_entities {
            if let Some(id) = entity.take() {
                if world.exists(id) {
                    let _ = world.despawn(id);
                }
            }
        }
        self.armed_handle = None;
        self.drag = None;
    }
}

fn editor_gizmo_render_options() -> MeshRenderOptions {
    MeshRenderOptions {
        role: MeshRenderRole::EditorGizmo,
        transform_policy: MeshTransformPolicy::World,
        visibility_policy: MeshVisibilityPolicy::EditorOnly,
        depth_policy: MeshDepthPolicy::Disabled,
        shadow_policy: MeshShadowPolicy::None,
        cull_policy: MeshCullPolicy::None,
        sort_policy: MeshSortPolicy::DebugLast,
    }
}

fn gizmo_primitive_for_mode(mode: UiEditorTransformMode) -> PrimitiveId {
    match mode {
        UiEditorTransformMode::Rotate => builtins::ID_TORUS,
        UiEditorTransformMode::Scale => builtins::ID_CUBE,
        UiEditorTransformMode::Translate | UiEditorTransformMode::Select => builtins::ID_CYLINDER,
    }
}

fn gizmo_handle_transform(
    mode: UiEditorTransformMode,
    handle: EditorGizmoHandle,
    origin: Vec3,
    orientation: Quat,
    length: f32,
    thickness: f32,
) -> Transform {
    match handle {
        EditorGizmoHandle::Axis(axis) => {
            gizmo_axis_transform(mode, axis, origin, orientation, length, thickness)
        }
        EditorGizmoHandle::Plane(plane) => {
            let (a, b) = plane.basis();
            let local_center = (a + b) * (length * 0.22);
            let plane_size = (length * 0.16).max(thickness * 2.0);
            let thin = (thickness * 0.35).max(0.008);
            let scale = match plane {
                EditorGizmoPlane::XY => Vec3::new(plane_size, plane_size, thin),
                EditorGizmoPlane::XZ => Vec3::new(plane_size, thin, plane_size),
                EditorGizmoPlane::YZ => Vec3::new(thin, plane_size, plane_size),
            };
            Transform {
                position: origin + orientation * local_center,
                rotation: orientation,
                scale,
            }
        }
        EditorGizmoHandle::Center => Transform {
            position: origin,
            rotation: orientation,
            scale: Vec3::splat(if mode == UiEditorTransformMode::Scale {
                (length * 0.14).max(thickness * 2.4)
            } else {
                (length * 0.10).max(thickness * 2.0)
            }),
        },
    }
}

fn gizmo_axis_transform(
    mode: UiEditorTransformMode,
    axis: EditorGizmoAxis,
    origin: Vec3,
    orientation: Quat,
    length: f32,
    thickness: f32,
) -> Transform {
    if mode == UiEditorTransformMode::Rotate {
        let rotation = match axis {
            EditorGizmoAxis::X => Quat::from_rotation_z(core::f32::consts::FRAC_PI_2),
            EditorGizmoAxis::Y => Quat::IDENTITY,
            EditorGizmoAxis::Z => Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
        };
        return Transform {
            position: origin,
            rotation: orientation * rotation,
            scale: Vec3::splat(length * 0.72),
        };
    }

    let axis_vec = axis.vector();
    let world_axis = (orientation * axis_vec).normalize_or_zero();
    if mode == UiEditorTransformMode::Scale {
        let scale = match axis {
            EditorGizmoAxis::X => Vec3::new(length, thickness * 2.4, thickness * 2.4),
            EditorGizmoAxis::Y => Vec3::new(thickness * 2.4, length, thickness * 2.4),
            EditorGizmoAxis::Z => Vec3::new(thickness * 2.4, thickness * 2.4, length),
        };
        return Transform {
            position: origin + world_axis * (length * 0.5),
            rotation: orientation,
            scale,
        };
    }
    let rotation = match axis {
        EditorGizmoAxis::X => Quat::from_rotation_z(-core::f32::consts::FRAC_PI_2),
        EditorGizmoAxis::Y => Quat::IDENTITY,
        EditorGizmoAxis::Z => Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
    };
    Transform {
        position: origin + world_axis * (length * 0.5),
        rotation: orientation * rotation,
        scale: Vec3::new(thickness, length, thickness),
    }
}
