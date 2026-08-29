#![forbid(unsafe_op_in_unsafe_fn)]

//! Focused editor viewport state machine and policy.
//! Scene/material/physics admission belongs to the composition adapter, not here.

mod camera;
mod gizmo;
mod transform;
mod types;

use newengine_ui_api::{
    UiEditorViewportProjection, UiEditorViewportShading, UiEditorViewportState,
};
use types::{ActiveTransformDrag, TransformTransaction};

pub use camera::EditorCameraProjectionOverride;
pub use gizmo::EditorGizmoSpec;
pub use transform::EditorTransformEffects;
pub use types::{
    EditorGizmoAxis, EditorGizmoAxisComponent, EditorGizmoHandle, EditorGizmoPlane,
    GIZMO_HANDLE_COUNT,
};

pub struct EditorViewportController {
    active: bool,
    pub(crate) state: UiEditorViewportState,
    pub(crate) armed_handle: Option<EditorGizmoHandle>,
    pub(crate) drag: Option<ActiveTransformDrag>,
    pub(crate) undo: Vec<TransformTransaction>,
    pub(crate) redo: Vec<TransformTransaction>,
    pub(crate) orthographic_half_height: f32,
    pub(crate) last_projection: UiEditorViewportProjection,
    pub(crate) inspector_dirty: bool,
}
impl Default for EditorViewportController {
    fn default() -> Self {
        Self {
            active: false,
            state: UiEditorViewportState::default(),
            armed_handle: None,
            drag: None,
            undo: Vec::new(),
            redo: Vec::new(),
            orthographic_half_height: 10.0,
            last_projection: UiEditorViewportProjection::Perspective,
            inspector_dirty: false,
        }
    }
}
impl EditorViewportController {
    #[inline]
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.armed_handle = None;
            self.drag = None;
        }
    }
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }
    #[inline]
    pub fn sync_state(&mut self, state: UiEditorViewportState) {
        if self.last_projection != state.projection {
            self.last_projection = state.projection;
            self.orthographic_half_height =
                if state.projection == UiEditorViewportProjection::Perspective {
                    10.0
                } else {
                    0.0
                };
        }
        self.state = state;
    }
    #[inline]
    pub fn state(&self) -> &UiEditorViewportState {
        &self.state
    }
    #[inline]
    pub fn shading(&self) -> UiEditorViewportShading {
        self.state.shading
    }
    #[inline]
    pub fn arm_gizmo_handle(&mut self, handle: EditorGizmoHandle) {
        self.armed_handle = Some(handle);
    }
    #[inline]
    pub fn clear_gizmo_handle(&mut self) {
        if self.drag.is_none() {
            self.armed_handle = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transformed_from_drag;
    use newengine_ecs::EntityId;
    use newengine_math::{Mat4, Quat, Vec3};
    use newengine_transform::Transform;
    use newengine_ui_api::UiEditorTransformMode;

    #[test]
    fn translation_snap_is_transaction_relative() {
        let before = Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            ..Transform::default()
        };
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Axis(EditorGizmoAxis::X),
            axis_vector: Vec3::X,
            plane_a: Vec3::ZERO,
            plane_b: Vec3::ZERO,
            before,
            world_origin: before.position,
            parent_world_inverse: Mat4::IDENTITY,
            parent_world_rotation: Quat::IDENTITY,
            accumulated: 14.9,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Translate,
            translation_snap_enabled: true,
            translation_snap_units: 10.0,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        assert_eq!(out.position, Vec3::new(11.0, 2.0, 3.0));
    }
    #[test]
    fn scale_never_crosses_zero() {
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Axis(EditorGizmoAxis::Y),
            axis_vector: Vec3::Y,
            plane_a: Vec3::ZERO,
            plane_b: Vec3::ZERO,
            before: Transform::default(),
            world_origin: Vec3::ZERO,
            parent_world_inverse: Mat4::IDENTITY,
            parent_world_rotation: Quat::IDENTITY,
            accumulated: -5.0,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Scale,
            ..UiEditorViewportState::default()
        };
        assert!(transformed_from_drag(drag, &state).scale.y > 0.0);
    }
    #[test]
    fn planar_translation_moves_only_plane_axes() {
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Plane(EditorGizmoPlane::XY),
            axis_vector: Vec3::ZERO,
            plane_a: Vec3::X,
            plane_b: Vec3::Y,
            before: Transform::default(),
            world_origin: Vec3::ZERO,
            parent_world_inverse: Mat4::IDENTITY,
            parent_world_rotation: Quat::IDENTITY,
            accumulated: 0.0,
            accumulated_world: Vec3::new(12.0, -8.0, 99.0),
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Translate,
            translation_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        assert_eq!(
            transformed_from_drag(drag, &state).position,
            Vec3::new(12.0, -8.0, 0.0)
        );
    }
    #[test]
    fn center_scale_is_uniform() {
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Center,
            axis_vector: Vec3::ZERO,
            plane_a: Vec3::ZERO,
            plane_b: Vec3::ZERO,
            before: Transform::default(),
            world_origin: Vec3::ZERO,
            parent_world_inverse: Mat4::IDENTITY,
            parent_world_rotation: Quat::IDENTITY,
            accumulated: 0.5,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Scale,
            scale_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        assert_eq!(transformed_from_drag(drag, &state).scale, Vec3::splat(1.5));
    }
    #[test]
    fn world_translation_is_converted_into_parent_local_space() {
        let parent = Mat4::from_quat(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2));
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Axis(EditorGizmoAxis::X),
            axis_vector: Vec3::X,
            plane_a: Vec3::ZERO,
            plane_b: Vec3::ZERO,
            before: Transform::default(),
            world_origin: Vec3::ZERO,
            parent_world_inverse: parent.inverse(),
            parent_world_rotation: Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
            accumulated: 1.0,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Translate,
            translation_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        let world_delta = parent.transform_vector3(out.position);
        assert!((world_delta - Vec3::X).length() < 1.0e-5);
    }

    #[test]
    fn world_rotation_is_converted_into_parent_local_rotation() {
        let parent_rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_2);
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            handle: EditorGizmoHandle::Axis(EditorGizmoAxis::X),
            axis_vector: Vec3::X,
            plane_a: Vec3::ZERO,
            plane_b: Vec3::ZERO,
            before: Transform::default(),
            world_origin: Vec3::ZERO,
            parent_world_inverse: Mat4::from_quat(parent_rotation).inverse(),
            parent_world_rotation: parent_rotation,
            accumulated: 90.0,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Rotate,
            rotation_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        let actual_world = (parent_rotation * out.rotation).normalize();
        let expected_world =
            (Quat::from_rotation_x(core::f32::consts::FRAC_PI_2) * parent_rotation).normalize();
        assert!(actual_world.dot(expected_world).abs() > 0.999_999);
    }
}
