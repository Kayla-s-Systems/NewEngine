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
use newengine_transform::{GlobalTransform, Transform};
use newengine_ui_api::{
    UiEditorTransformMode, UiEditorTransformSpace, UiEditorViewportProjection,
    UiEditorViewportShading, UiEditorViewportState, UiInputFrame,
};

use crate::gameplay::{DisplayMode, DisplayVisibility};
use crate::scene_bridge::{
    apply_primitive_instance, ensure_primitive_base, primitive_bounds, EngineViewGatewayFrame,
    SceneBridge,
};

const EDITOR_HISTORY_LIMIT: usize = 256;
const GIZMO_HANDLE_COUNT: usize = 7;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoPlane {
    XY,
    XZ,
    YZ,
}

impl EditorGizmoPlane {
    #[inline]
    const fn basis(self) -> (Vec3, Vec3) {
        match self {
            Self::XY => (Vec3::X, Vec3::Y),
            Self::XZ => (Vec3::X, Vec3::Z),
            Self::YZ => (Vec3::Y, Vec3::Z),
        }
    }

    #[inline]
    const fn name(self) -> &'static str {
        match self {
            Self::XY => "XY",
            Self::XZ => "XZ",
            Self::YZ => "YZ",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoHandle {
    Axis(EditorGizmoAxis),
    Plane(EditorGizmoPlane),
    Center,
}

impl EditorGizmoHandle {
    #[inline]
    const fn index(self) -> usize {
        match self {
            Self::Axis(axis) => axis.index(),
            Self::Plane(EditorGizmoPlane::XY) => 3,
            Self::Plane(EditorGizmoPlane::XZ) => 4,
            Self::Plane(EditorGizmoPlane::YZ) => 5,
            Self::Center => 6,
        }
    }

    #[inline]
    const fn name(self) -> &'static str {
        match self {
            Self::Axis(axis) => axis.name(),
            Self::Plane(plane) => plane.name(),
            Self::Center => "Center",
        }
    }

    #[inline]
    const fn color(self) -> [f32; 4] {
        match self {
            Self::Axis(axis) => axis.color(),
            Self::Plane(EditorGizmoPlane::XY) => [0.88, 0.78, 0.18, 0.92],
            Self::Plane(EditorGizmoPlane::XZ) => [0.78, 0.22, 0.82, 0.92],
            Self::Plane(EditorGizmoPlane::YZ) => [0.16, 0.76, 0.78, 0.92],
            Self::Center => [0.92, 0.92, 0.92, 1.0],
        }
    }
}

/// Runtime-only component attached to editor gizmo geometry.
/// It deliberately never becomes authored scene data.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorGizmoAxisComponent {
    pub(crate) handle: EditorGizmoHandle,
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
    handle: EditorGizmoHandle,
    axis_vector: Vec3,
    plane_a: Vec3,
    plane_b: Vec3,
    before: Transform,
    accumulated: f32,
    accumulated_world: Vec3,
}

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

    pub(crate) fn process_history_actions(
        &mut self,
        world: &mut World,
        dispatch: Option<&newengine_ui_api::UiEventDispatchFrame>,
    ) {
        let Some(dispatch) = dispatch else {
            return;
        };
        for action in &dispatch.actions {
            if action.trigger != newengine_ui_api::UiNodeEventTrigger::Click {
                continue;
            }
            match action.action_id.as_str() {
                "editor.history.undo" => self.undo(world),
                "editor.history.redo" => self.redo(world),
                _ => {}
            }
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

        let text_input_active = !input.text.is_empty()
            || !input.text_edit_ops.is_empty()
            || !input.ime_preedit.is_empty();
        if text_input_active && self.drag.is_none() {
            return;
        }

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
            if let (Some(entity), Some(handle)) = (selected, self.armed_handle) {
                if let Some(before) = world.get::<Transform>(entity).copied() {
                    let rotate_basis = |axis: Vec3| {
                        match self.state.transform_space {
                            UiEditorTransformSpace::World => axis,
                            UiEditorTransformSpace::Local => before.rotation * axis,
                        }
                        .normalize_or_zero()
                    };
                    let (axis_vector, plane_a, plane_b) = match handle {
                        EditorGizmoHandle::Axis(axis) => {
                            (rotate_basis(axis.vector()), Vec3::ZERO, Vec3::ZERO)
                        }
                        EditorGizmoHandle::Plane(plane) => {
                            let (a, b) = plane.basis();
                            (Vec3::ZERO, rotate_basis(a), rotate_basis(b))
                        }
                        EditorGizmoHandle::Center => (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
                    };
                    self.drag = Some(ActiveTransformDrag {
                        entity,
                        handle,
                        axis_vector,
                        plane_a,
                        plane_b,
                        before,
                        accumulated: 0.0,
                        accumulated_world: Vec3::ZERO,
                    });
                }
            }
        }

        if let Some(mut drag) = self.drag {
            if left_released || !left_down {
                self.commit_drag(world, drag);
                self.drag = None;
                self.armed_handle = None;
                return;
            }

            if input.mouse_delta != (0.0, 0.0) {
                match drag.handle {
                    EditorGizmoHandle::Axis(_) => {
                        drag.accumulated += transform_drag_sensitivity(
                            camera,
                            drag.before.position,
                            drag.axis_vector,
                            input.mouse_delta,
                            viewport_size,
                            self.state.projection,
                            self.orthographic_half_height,
                        );
                    }
                    EditorGizmoHandle::Plane(_) => {
                        let delta = transform_screen_world_delta(
                            camera,
                            drag.before.position,
                            input.mouse_delta,
                            viewport_size,
                            self.state.projection,
                            self.orthographic_half_height,
                        );
                        drag.accumulated_world += drag.plane_a * delta.dot(drag.plane_a)
                            + drag.plane_b * delta.dot(drag.plane_b);
                    }
                    EditorGizmoHandle::Center => match self.state.transform_mode {
                        UiEditorTransformMode::Translate => {
                            drag.accumulated_world += transform_screen_world_delta(
                                camera,
                                drag.before.position,
                                input.mouse_delta,
                                viewport_size,
                                self.state.projection,
                                self.orthographic_half_height,
                            );
                        }
                        UiEditorTransformMode::Scale => {
                            drag.accumulated += (input.mouse_delta.0 - input.mouse_delta.1) * 0.01;
                        }
                        _ => {}
                    },
                }
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
            drag.handle.name(),
            self.undo.len(),
        );
    }

    fn cancel_drag(&mut self, world: &mut World) {
        let Some(drag) = self.drag.take() else {
            self.armed_handle = None;
            return;
        };
        if let Some(transform) = world.get_mut_tracked::<Transform>(drag.entity) {
            *transform = drag.before;
            self.inspector_dirty = true;
        }
        self.armed_handle = None;
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
            .collect::<std::collections::BTreeSet<_>>();
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
        if self.last_projection != self.state.projection
            || !self.orthographic_half_height.is_finite()
        {
            self.orthographic_half_height = (radius * 1.75).clamp(0.5, 10_000.0);
            self.last_projection = self.state.projection;
        }
        if wheel_y.abs() > f32::EPSILON {
            self.orthographic_half_height =
                (self.orthographic_half_height * (-wheel_y * 0.12).exp()).clamp(0.05, 100_000.0);
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
        snapshot.finite = view_projection
            .to_cols_array()
            .iter()
            .all(|value| value.is_finite());
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

fn transformed_from_drag(drag: ActiveTransformDrag, state: &UiEditorViewportState) -> Transform {
    match state.transform_mode {
        UiEditorTransformMode::Select => drag.before,
        UiEditorTransformMode::Translate => {
            let step = state.translation_snap_units.max(0.0001);
            let delta = match drag.handle {
                EditorGizmoHandle::Axis(_) => {
                    let mut distance = drag.accumulated;
                    if state.translation_snap_enabled {
                        distance = snap_scalar(distance, step);
                    }
                    drag.axis_vector * distance
                }
                EditorGizmoHandle::Plane(_) => {
                    let mut a = drag.accumulated_world.dot(drag.plane_a);
                    let mut b = drag.accumulated_world.dot(drag.plane_b);
                    if state.translation_snap_enabled {
                        a = snap_scalar(a, step);
                        b = snap_scalar(b, step);
                    }
                    drag.plane_a * a + drag.plane_b * b
                }
                EditorGizmoHandle::Center => {
                    if state.translation_snap_enabled {
                        Vec3::new(
                            snap_scalar(drag.accumulated_world.x, step),
                            snap_scalar(drag.accumulated_world.y, step),
                            snap_scalar(drag.accumulated_world.z, step),
                        )
                    } else {
                        drag.accumulated_world
                    }
                }
            };
            Transform {
                position: drag.before.position + delta,
                ..drag.before
            }
        }
        UiEditorTransformMode::Rotate => {
            let EditorGizmoHandle::Axis(axis) = drag.handle else {
                return drag.before;
            };
            let mut degrees = drag.accumulated;
            if state.rotation_snap_enabled {
                degrees = snap_scalar(degrees, state.rotation_snap_degrees.max(0.0001));
            }
            let rotation_axis = match state.transform_space {
                UiEditorTransformSpace::World => drag.axis_vector,
                UiEditorTransformSpace::Local => axis.vector(),
            };
            let delta = Quat::from_axis_angle(rotation_axis, degrees.to_radians());
            Transform {
                rotation: match state.transform_space {
                    UiEditorTransformSpace::World => delta * drag.before.rotation,
                    UiEditorTransformSpace::Local => drag.before.rotation * delta,
                },
                ..drag.before
            }
        }
        UiEditorTransformMode::Scale => {
            let mut delta = drag.accumulated;
            if state.scale_snap_enabled {
                delta = snap_scalar(delta, (state.scale_snap_percent / 100.0).max(0.0001));
            }
            let mut scale = match drag.handle {
                EditorGizmoHandle::Axis(axis) => drag.before.scale + axis.vector() * delta,
                EditorGizmoHandle::Center => drag.before.scale + Vec3::splat(delta),
                EditorGizmoHandle::Plane(_) => drag.before.scale,
            };
            scale.x = scale.x.max(0.001);
            scale.y = scale.y.max(0.001);
            scale.z = scale.z.max(0.001);
            Transform {
                scale,
                ..drag.before
            }
        }
    }
}

fn transform_screen_world_delta(
    camera: Option<&CameraFrameSnapshot>,
    object_position: Vec3,
    mouse_delta: (f32, f32),
    viewport_size: [u32; 2],
    projection: UiEditorViewportProjection,
    orthographic_half_height: f32,
) -> Vec3 {
    let Some(camera) = camera else {
        return Vec3::new(mouse_delta.0, -mouse_delta.1, 0.0) * 0.01;
    };
    let right =
        Vec3::new(camera.right_ws[0], camera.right_ws[1], camera.right_ws[2]).normalize_or_zero();
    let up = Vec3::new(camera.up_ws[0], camera.up_ws[1], camera.up_ws[2]).normalize_or_zero();
    let world_per_pixel = match projection {
        UiEditorViewportProjection::Perspective => {
            let camera_position = Vec3::new(
                camera.position_ws[0],
                camera.position_ws[1],
                camera.position_ws[2],
            );
            let distance = object_position.distance(camera_position).max(0.1);
            let fovy = camera.projection.fovy.max(1.0_f32.to_radians());
            2.0 * distance * (fovy * 0.5).tan() / viewport_size[1].max(1) as f32
        }
        _ => 2.0 * orthographic_half_height.max(0.01) / viewport_size[1].max(1) as f32,
    };
    (right * mouse_delta.0 - up * mouse_delta.1) * world_per_pixel
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
    let right =
        Vec3::new(camera.right_ws[0], camera.right_ws[1], camera.right_ws[2]).normalize_or_zero();
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
            let camera_position = Vec3::new(
                camera.position_ws[0],
                camera.position_ws[1],
                camera.position_ws[2],
            );
            let distance = object_position.distance(camera_position).max(0.1);
            let fovy = camera.projection.fovy.max(1.0_f32.to_radians());
            let world_per_pixel =
                2.0 * distance * (fovy * 0.5).tan() / viewport_size[1].max(1) as f32;
            projected_delta * world_per_pixel
        }
        _ => {
            let world_per_pixel =
                2.0 * orthographic_half_height.max(0.01) / viewport_size[1].max(1) as f32;
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
