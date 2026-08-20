#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{Orthographic, Projection};
use newengine_camera_api::{CameraFrameSnapshot, CameraProjectionKind};
use newengine_ecs::{EntityId, World};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_domain_api::{
    MeshCullPolicy, MeshDepthPolicy, MeshRenderOptions, MeshRenderRole, MeshShadowPolicy,
    MeshSortPolicy, MeshTransformPolicy, MeshVisibilityPolicy,
};
use newengine_primitives::{builtins, Primitive, PrimitiveId};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::Transform;
use newengine_ui_api::{
    UiEditorTransformMode, UiEditorViewportProjection, UiEditorViewportShading,
    UiEditorViewportState, UiInputFrame,
};

use crate::gameplay::{DisplayMode, DisplayVisibility};
use crate::scene_bridge::{
    apply_primitive_instance, ensure_primitive_base, primitive_bounds, EngineViewGatewayFrame,
    SceneBridge,
};

const EDITOR_HISTORY_LIMIT: usize = 256;
const GIZMO_AXIS_COUNT: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoAxis {
    X,
    Y,
    Z,
}

impl EditorGizmoAxis {
    #[inline]
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    #[inline]
    pub(crate) const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    #[inline]
    const fn name(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    #[inline]
    const fn color(self) -> [f32; 4] {
        match self {
            Self::X => [0.92, 0.16, 0.14, 1.0],
            Self::Y => [0.18, 0.78, 0.28, 1.0],
            Self::Z => [0.18, 0.42, 0.96, 1.0],
        }
    }
}

/// Runtime-only component attached to editor gizmo geometry.
/// It deliberately never becomes authored scene data.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorGizmoAxisComponent {
    pub(crate) axis: EditorGizmoAxis,
}

#[derive(Clone, Copy, Debug)]
struct TransformTransaction {
    entity: EntityId,
    before: Transform,
    after: Transform,
}

#[derive(Clone, Copy, Debug)]
struct ActiveTransformDrag {
    entity: EntityId,
    axis: EditorGizmoAxis,
    before: Transform,
    accumulated: f32,
}

pub(crate) struct EditorViewportController {
    active: bool,
    state: UiEditorViewportState,
    armed_axis: Option<EditorGizmoAxis>,
    drag: Option<ActiveTransformDrag>,
    undo: Vec<TransformTransaction>,
    redo: Vec<TransformTransaction>,
    gizmo_entities: [Option<EntityId>; GIZMO_AXIS_COUNT],
    orthographic_half_height: f32,
    last_projection: UiEditorViewportProjection,
    inspector_dirty: bool,
}

impl Default for EditorViewportController {
    fn default() -> Self {
        Self {
            active: false,
            state: UiEditorViewportState::default(),
            armed_axis: None,
            drag: None,
            undo: Vec::new(),
            redo: Vec::new(),
            gizmo_entities: [None; GIZMO_AXIS_COUNT],
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
            self.armed_axis = None;
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
            self.orthographic_half_height = if state.projection == UiEditorViewportProjection::Perspective {
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
    pub(crate) fn arm_gizmo_axis(&mut self, axis: EditorGizmoAxis) {
        self.armed_axis = Some(axis);
    }

    #[inline]
    pub(crate) fn clear_gizmo_axis(&mut self) {
        if self.drag.is_none() {
            self.armed_axis = None;
        }
    }

    pub(crate) fn process_transform_input(
        &mut self,
        world: &mut World,
        selected: Option<EntityId>,
        input: Option<&UiInputFrame>,
        camera: Option<&CameraFrameSnapshot>,
        viewport_size: [u32; 2],
    ) {
        let Some(input) = input else {
            return;
        };

        let control_down = input.is_key_down(newengine_input_api::key_code::CONTROL_LEFT)
            || input.is_key_down(newengine_input_api::key_code::CONTROL_RIGHT);
        if control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_Z) {
            self.undo(world);
        } else if control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_Y) {
            self.redo(world);
        }

        if input.is_key_pressed(newengine_input_api::key_code::ESCAPE) {
            self.cancel_drag(world);
            return;
        }

        let left_down = input.is_mouse_down(newengine_input_api::mouse_button::LEFT);
        let left_released = input.is_mouse_released(newengine_input_api::mouse_button::LEFT);

        if self.drag.is_none() && left_down {
            if let (Some(entity), Some(axis)) = (selected, self.armed_axis) {
                if let Some(before) = world.get::<Transform>(entity).copied() {
                    self.drag = Some(ActiveTransformDrag {
                        entity,
                        axis,
                        before,
                        accumulated: 0.0,
                    });
                }
            }
        }

        if let Some(mut drag) = self.drag {
            if left_released || !left_down {
                self.commit_drag(world, drag);
                self.drag = None;
                self.armed_axis = None;
                return;
            }

            if input.mouse_delta != (0.0, 0.0) {
                let sensitivity = transform_drag_sensitivity(
                    camera,
                    drag.before.position,
                    drag.axis.vector(),
                    input.mouse_delta,
                    viewport_size,
                    self.state.projection,
                    self.orthographic_half_height,
                );
                drag.accumulated += sensitivity;
                let next = transformed_from_drag(drag, &self.state);
                if let Some(transform) = world.get_mut_tracked::<Transform>(drag.entity) {
                    *transform = next;
                    self.inspector_dirty = true;
                }
                self.drag = Some(drag);
            }
        }
    }

    fn commit_drag(&mut self, world: &World, drag: ActiveTransformDrag) {
        let Some(after) = world.get::<Transform>(drag.entity).copied() else {
            return;
        };
        if transforms_approximately_equal(drag.before, after) {
            return;
        }
        if self.undo.len() >= EDITOR_HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(TransformTransaction {
            entity: drag.entity,
            before: drag.before,
            after,
        });
        self.redo.clear();
        self.inspector_dirty = true;
        newengine_ulog_api::ulog::info!(
            "editor transform transaction: committed entity={} mode={} axis={} undo_depth={}",
            drag.entity.stable_u64(),
            self.state.transform_mode.label(),
            drag.axis.name(),
            self.undo.len(),
        );
    }

    fn cancel_drag(&mut self, world: &mut World) {
        let Some(drag) = self.drag.take() else {
            self.armed_axis = None;
            return;
        };
        if let Some(transform) = world.get_mut_tracked::<Transform>(drag.entity) {
            *transform = drag.before;
            self.inspector_dirty = true;
        }
        self.armed_axis = None;
    }

    fn undo(&mut self, world: &mut World) {
        let Some(transaction) = self.undo.pop() else {
            return;
        };
        if let Some(transform) = world.get_mut_tracked::<Transform>(transaction.entity) {
            *transform = transaction.before;
            self.redo.push(transaction);
            self.inspector_dirty = true;
            newengine_ulog_api::ulog::info!(
                "editor transform transaction: undo entity={} undo_depth={} redo_depth={}",
                transaction.entity.stable_u64(),
                self.undo.len(),
                self.redo.len(),
            );
        }
    }

    fn redo(&mut self, world: &mut World) {
        let Some(transaction) = self.redo.pop() else {
            return;
        };
        if let Some(transform) = world.get_mut_tracked::<Transform>(transaction.entity) {
            *transform = transaction.after;
            self.undo.push(transaction);
            self.inspector_dirty = true;
            newengine_ulog_api::ulog::info!(
                "editor transform transaction: redo entity={} undo_depth={} redo_depth={}",
                transaction.entity.stable_u64(),
                self.undo.len(),
                self.redo.len(),
            );
        }
    }

    #[inline]
    pub(crate) fn take_inspector_dirty(&mut self) -> bool {
        core::mem::take(&mut self.inspector_dirty)
    }

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

        let length = (selection_radius.max(0.25) * 1.8).clamp(0.65, 8.0);
        let thickness = (length * 0.045).clamp(0.025, 0.18);
        let primitive = gizmo_primitive_for_mode(self.state.transform_mode);

        let mats_lock = scene_bridge.materials();
        let mats = mats_lock.read();
        let default_mat = mats.register_named(
            "EditorGizmo",
            newengine_materials::MaterialDescriptor::default(),
        );
        let prims_lock = scene_bridge.primitives();
        let prims = prims_lock.read();
        let world = scene.world_mut();

        for axis in [EditorGizmoAxis::X, EditorGizmoAxis::Y, EditorGizmoAxis::Z] {
            let index = axis.index();
            let entity = match self.gizmo_entities[index].filter(|entity| world.exists(*entity)) {
                Some(entity) => entity,
                None => {
                    let entity = spawn_named(world, format!("__EditorGizmo{}", axis.name()));
                    self.gizmo_entities[index] = Some(entity);
                    entity
                }
            };

            let color = axis.color();
            let _ = world.insert(entity, Primitive { id: primitive, color });
            let _ = world.insert(entity, EditorGizmoAxisComponent { axis });
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

            let transform = gizmo_axis_transform(
                self.state.transform_mode,
                axis,
                selected_transform.position,
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
        self.armed_axis = None;
        self.drag = None;
    }

    pub(crate) fn apply_camera_projection(
        &mut self,
        frame: &mut EngineViewGatewayFrame,
        world_center: Vec3,
        world_radius: f32,
        selection_center: Option<Vec3>,
        selection_radius: Option<f32>,
        wheel_y: f32,
        viewport_size: [u32; 2],
    ) {
        if self.state.projection == UiEditorViewportProjection::Perspective {
            return;
        }

        let center = selection_center.unwrap_or(world_center);
        let radius = selection_radius.unwrap_or(world_radius).max(0.25);
        if self.last_projection != self.state.projection || !self.orthographic_half_height.is_finite() {
            self.orthographic_half_height = (radius * 1.75).clamp(0.5, 10_000.0);
            self.last_projection = self.state.projection;
        }
        if wheel_y.abs() > f32::EPSILON {
            self.orthographic_half_height = (self.orthographic_half_height
                * (-wheel_y * 0.12).exp())
                .clamp(0.05, 100_000.0);
        } else if self.orthographic_half_height <= 0.0 {
            self.orthographic_half_height = (radius * 1.75).max(0.5);
        }

        let distance = (radius * 6.0).max(10.0);
        let (eye, up) = match self.state.projection {
            UiEditorViewportProjection::Top => (center + Vec3::Y * distance, -Vec3::Z),
            UiEditorViewportProjection::Front => (center + Vec3::Z * distance, Vec3::Y),
            UiEditorViewportProjection::Side => (center + Vec3::X * distance, Vec3::Y),
            UiEditorViewportProjection::Perspective => return,
        };
        let width = viewport_size[0].max(1);
        let height = viewport_size[1].max(1);
        let aspect = width as f32 / height as f32;
        let near = 0.01;
        let far = (distance + world_radius.max(radius) * 8.0 + 100.0).max(1_000.0);
        let view = Mat4::look_at_rh(eye, center, up);
        let projection = Projection::Orthographic(Orthographic::new(
            self.orthographic_half_height,
            aspect,
            near,
            far,
        ))
        .matrix();
        let view_projection = projection * view;
        let inverse_view = view.inverse();
        let forward = (center - eye).normalize_or_zero();

        frame.view.view = view;
        frame.view.projection = projection;
        frame.view.view_projection = view_projection;
        frame.view.inverse_view = inverse_view;
        frame.view.position_ws = eye;
        frame.view.position_ws_f64 = [eye.x as f64, eye.y as f64, eye.z as f64];
        frame.view.position_origin_relative_ws = eye;
        frame.view.forward_ws = forward;
        frame.view.viewport_width = width;
        frame.view.viewport_height = height;
        frame.view.aspect = aspect;

        let snapshot = &mut frame.camera_snapshot;
        snapshot.view_cols = view.to_cols_array_2d();
        snapshot.projection_cols = projection.to_cols_array_2d();
        snapshot.view_projection_cols = view_projection.to_cols_array_2d();
        snapshot.inverse_view_cols = inverse_view.to_cols_array_2d();
        snapshot.inverse_projection_cols = projection.inverse().to_cols_array_2d();
        snapshot.inverse_view_projection_cols = view_projection.inverse().to_cols_array_2d();
        snapshot.position_ws = [eye.x, eye.y, eye.z];
        snapshot.position_ws_f64 = [eye.x as f64, eye.y as f64, eye.z as f64];
        snapshot.position_origin_relative_ws = [eye.x, eye.y, eye.z];
        snapshot.forward_ws = [forward.x, forward.y, forward.z];
        let right = inverse_view.x_axis.truncate().normalize_or_zero();
        let camera_up = inverse_view.y_axis.truncate().normalize_or_zero();
        snapshot.right_ws = [right.x, right.y, right.z];
        snapshot.up_ws = [camera_up.x, camera_up.y, camera_up.z];
        snapshot.viewport.width = width;
        snapshot.viewport.height = height;
        snapshot.viewport.aspect = aspect;
        snapshot.projection.kind = CameraProjectionKind::Orthographic;
        snapshot.projection.aspect = aspect;
        snapshot.projection.half_height = self.orthographic_half_height;
        snapshot.projection.near = near;
        snapshot.projection.far = far;
        snapshot.finite = view_projection.to_cols_array().iter().all(|value| value.is_finite());
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

fn gizmo_axis_transform(
    mode: UiEditorTransformMode,
    axis: EditorGizmoAxis,
    origin: Vec3,
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
            rotation,
            scale: Vec3::splat(length * 0.72),
        };
    }

    let axis_vec = axis.vector();
    let rotation = match axis {
        EditorGizmoAxis::X => Quat::from_rotation_z(-core::f32::consts::FRAC_PI_2),
        EditorGizmoAxis::Y => Quat::IDENTITY,
        EditorGizmoAxis::Z => Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
    };
    let scale = if mode == UiEditorTransformMode::Scale {
        match axis {
            EditorGizmoAxis::X => Vec3::new(length, thickness * 2.4, thickness * 2.4),
            EditorGizmoAxis::Y => Vec3::new(thickness * 2.4, length, thickness * 2.4),
            EditorGizmoAxis::Z => Vec3::new(thickness * 2.4, thickness * 2.4, length),
        }
    } else {
        Vec3::new(thickness, length, thickness)
    };
    Transform {
        position: origin + axis_vec * (length * 0.5),
        rotation,
        scale,
    }
}

fn transformed_from_drag(drag: ActiveTransformDrag, state: &UiEditorViewportState) -> Transform {
    let axis = drag.axis.vector();
    match state.transform_mode {
        UiEditorTransformMode::Select => drag.before,
        UiEditorTransformMode::Translate => {
            let mut distance = drag.accumulated;
            if state.translation_snap_enabled {
                distance = snap_scalar(distance, state.translation_snap_units.max(0.0001));
            }
            Transform {
                position: drag.before.position + axis * distance,
                ..drag.before
            }
        }
        UiEditorTransformMode::Rotate => {
            let mut degrees = drag.accumulated;
            if state.rotation_snap_enabled {
                degrees = snap_scalar(degrees, state.rotation_snap_degrees.max(0.0001));
            }
            Transform {
                rotation: Quat::from_axis_angle(axis, degrees.to_radians()) * drag.before.rotation,
                ..drag.before
            }
        }
        UiEditorTransformMode::Scale => {
            let mut delta = drag.accumulated;
            if state.scale_snap_enabled {
                delta = snap_scalar(delta, (state.scale_snap_percent / 100.0).max(0.0001));
            }
            let mut scale = drag.before.scale + axis * delta;
            scale.x = scale.x.max(0.001);
            scale.y = scale.y.max(0.001);
            scale.z = scale.z.max(0.001);
            Transform { scale, ..drag.before }
        }
    }
}

fn transform_drag_sensitivity(
    camera: Option<&CameraFrameSnapshot>,
    object_position: Vec3,
    axis: Vec3,
    mouse_delta: (f32, f32),
    viewport_size: [u32; 2],
    projection: UiEditorViewportProjection,
    orthographic_half_height: f32,
) -> f32 {
    let Some(camera) = camera else {
        return (mouse_delta.0 - mouse_delta.1) * 0.01;
    };
    let right = Vec3::new(camera.right_ws[0], camera.right_ws[1], camera.right_ws[2]).normalize_or_zero();
    let up = Vec3::new(camera.up_ws[0], camera.up_ws[1], camera.up_ws[2]).normalize_or_zero();
    let screen_axis = [axis.dot(right), -axis.dot(up)];
    let screen_len = (screen_axis[0] * screen_axis[0] + screen_axis[1] * screen_axis[1]).sqrt();
    let projected_delta = if screen_len > 1.0e-4 {
        (mouse_delta.0 * screen_axis[0] + mouse_delta.1 * screen_axis[1]) / screen_len
    } else {
        mouse_delta.0 - mouse_delta.1
    };

    match projection {
        UiEditorViewportProjection::Perspective => {
            let camera_position = Vec3::new(camera.position_ws[0], camera.position_ws[1], camera.position_ws[2]);
            let distance = object_position.distance(camera_position).max(0.1);
            let fovy = camera.projection.fovy.max(1.0_f32.to_radians());
            let world_per_pixel = 2.0 * distance * (fovy * 0.5).tan()
                / viewport_size[1].max(1) as f32;
            projected_delta * world_per_pixel
        }
        _ => {
            let world_per_pixel = 2.0 * orthographic_half_height.max(0.01)
                / viewport_size[1].max(1) as f32;
            projected_delta * world_per_pixel
        }
    }
}

#[inline]
fn snap_scalar(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

#[inline]
fn transforms_approximately_equal(a: Transform, b: Transform) -> bool {
    a.position.distance_squared(b.position) <= 1.0e-10
        && a.scale.distance_squared(b.scale) <= 1.0e-10
        && a.rotation.dot(b.rotation).abs() >= 0.999_999
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_snap_is_transaction_relative() {
        let before = Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            ..Transform::default()
        };
        let drag = ActiveTransformDrag {
            entity: EntityId::default(),
            axis: EditorGizmoAxis::X,
            before,
            accumulated: 14.9,
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
            axis: EditorGizmoAxis::Y,
            before: Transform::default(),
            accumulated: -5.0,
        };
        let state = UiEditorViewportState {
            transform_mode: UiEditorTransformMode::Scale,
            ..UiEditorViewportState::default()
        };
        let out = transformed_from_drag(drag, &state);
        assert!(out.scale.y > 0.0);
    }
}
