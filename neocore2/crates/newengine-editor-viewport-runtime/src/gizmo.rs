use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_model_domain_api::{
    MeshCullPolicy, MeshDepthPolicy, MeshRenderOptions, MeshRenderRole, MeshShadowPolicy,
    MeshSortPolicy, MeshTransformPolicy, MeshVisibilityPolicy,
};
use newengine_primitives::{builtins, PrimitiveId};
use newengine_transform::{GlobalTransform, Transform};
use newengine_ui_api::{UiEditorTransformMode, UiEditorTransformSpace};

use super::{EditorGizmoAxis, EditorGizmoHandle, EditorGizmoPlane, EditorViewportController};

#[derive(Clone, Debug)]
pub struct EditorGizmoSpec {
    pub handle: EditorGizmoHandle,
    pub primitive: PrimitiveId,
    pub color: [f32; 4],
    pub transform: Transform,
    pub render_options: MeshRenderOptions,
}

impl EditorViewportController {
    pub fn gizmo_specs(
        &self,
        world: &World,
        selected: Option<EntityId>,
        selection_radius: f32,
    ) -> Vec<EditorGizmoSpec> {
        let visible = self.state.gizmo_visible
            && self.state.transform_mode != UiEditorTransformMode::Select
            && selected.is_some();
        if !visible {
            return Vec::new();
        }
        let Some(selected) = selected else {
            return Vec::new();
        };
        let Some(selected_transform) = world.get::<Transform>(selected).copied() else {
            return Vec::new();
        };
        let (position, rotation) = world
            .get::<GlobalTransform>(selected)
            .map(|g| {
                let (_, r, t) = g.0.to_scale_rotation_translation();
                (t, r)
            })
            .unwrap_or((selected_transform.position, selected_transform.rotation));
        let orientation = match self.state.transform_space {
            UiEditorTransformSpace::World => Quat::IDENTITY,
            UiEditorTransformSpace::Local => rotation,
        };
        let length = (selection_radius.max(0.25) * 1.8).clamp(0.65, 8.0);
        let thickness = (length * 0.045).clamp(0.025, 0.18);
        let mut handles = vec![
            EditorGizmoHandle::Axis(EditorGizmoAxis::X),
            EditorGizmoHandle::Axis(EditorGizmoAxis::Y),
            EditorGizmoHandle::Axis(EditorGizmoAxis::Z),
        ];
        if self.state.transform_mode == UiEditorTransformMode::Translate {
            handles.extend([
                EditorGizmoHandle::Plane(EditorGizmoPlane::XY),
                EditorGizmoHandle::Plane(EditorGizmoPlane::XZ),
                EditorGizmoHandle::Plane(EditorGizmoPlane::YZ),
                EditorGizmoHandle::Center,
            ]);
        } else if self.state.transform_mode == UiEditorTransformMode::Scale {
            handles.push(EditorGizmoHandle::Center);
        }
        handles
            .into_iter()
            .map(|handle| {
                let primitive = match handle {
                    EditorGizmoHandle::Axis(_) => {
                        gizmo_primitive_for_mode(self.state.transform_mode)
                    }
                    EditorGizmoHandle::Plane(_) | EditorGizmoHandle::Center => builtins::ID_CUBE,
                };
                EditorGizmoSpec {
                    handle,
                    primitive,
                    color: handle.color(),
                    transform: gizmo_handle_transform(
                        self.state.transform_mode,
                        handle,
                        position,
                        orientation,
                        length,
                        thickness,
                    ),
                    render_options: editor_gizmo_render_options(),
                }
            })
            .collect()
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
            let local = (a + b) * (length * 0.22);
            let size = (length * 0.16).max(thickness * 2.0);
            let thin = (thickness * 0.35).max(0.008);
            let scale = match plane {
                EditorGizmoPlane::XY => Vec3::new(size, size, thin),
                EditorGizmoPlane::XZ => Vec3::new(size, thin, size),
                EditorGizmoPlane::YZ => Vec3::new(thin, size, size),
            };
            Transform {
                position: origin + orientation * local,
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
        let r = match axis {
            EditorGizmoAxis::X => Quat::from_rotation_z(core::f32::consts::FRAC_PI_2),
            EditorGizmoAxis::Y => Quat::IDENTITY,
            EditorGizmoAxis::Z => Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
        };
        return Transform {
            position: origin,
            rotation: orientation * r,
            scale: Vec3::splat(length * 0.72),
        };
    }
    let world_axis = (orientation * axis.vector()).normalize_or_zero();
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
