#![forbid(unsafe_op_in_unsafe_fn)]

mod camera;
mod gizmo;
mod transform;
mod types;

use newengine_ecs::EntityId;
use newengine_ui_api::{
    UiEditorViewportProjection, UiEditorViewportShading, UiEditorViewportState,
};

use types::*;
pub(crate) use types::{EditorGizmoAxisComponent, EditorGizmoHandle};

pub(crate) struct EditorViewportController {
    active: bool,
    state: UiEditorViewportState,
    armed_handle: Option<EditorGizmoHandle>,
    drag: Option<ActiveTransformDrag>,
    undo: Vec<TransformTransaction>,
    redo: Vec<TransformTransaction>,
    gizmo_entities: [Option<EntityId>; GIZMO_HANDLE_COUNT],
    orthographic_half_height: f32,
    last_projection: UiEditorViewportProjection,
    inspector_dirty: bool,
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
            gizmo_entities: [None; GIZMO_HANDLE_COUNT],
            orthographic_half_height: 10.0,
            last_projection: UiEditorViewportProjection::Perspective,
            inspector_dirty: false,
        }
    }
}

impl EditorViewportController {
    #[inline]
    pub(crate) fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.armed_handle = None;
            self.drag = None;
        }
    }

    #[inline]
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub(crate) fn sync_state(&mut self, state: UiEditorViewportState) {
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
    pub(crate) fn state(&self) -> &UiEditorViewportState {
        &self.state
    }

    #[inline]
    pub(crate) fn shading(&self) -> UiEditorViewportShading {
        self.state.shading
    }

    #[inline]
    pub(crate) fn arm_gizmo_handle(&mut self, handle: EditorGizmoHandle) {
        self.armed_handle = Some(handle);
    }

    #[inline]
    pub(crate) fn clear_gizmo_handle(&mut self) {
        if self.drag.is_none() {
            self.armed_handle = None;
        }
    }
}

#[cfg(test)]
use transform::transformed_from_drag;

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::Vec3;
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
            accumulated: -5.0,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Scale,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        assert!(out.scale.y > 0.0);
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
            accumulated: 0.0,
            accumulated_world: Vec3::new(12.0, -8.0, 99.0),
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Translate,
            translation_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        assert_eq!(out.position, Vec3::new(12.0, -8.0, 0.0));
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
            accumulated: 0.5,
            accumulated_world: Vec3::ZERO,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Scale,
            scale_snap_enabled: false,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        assert_eq!(out.scale, Vec3::splat(1.5));
    }
}
