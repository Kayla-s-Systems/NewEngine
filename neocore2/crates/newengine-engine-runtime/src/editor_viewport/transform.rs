use newengine_camera_api::CameraFrameSnapshot;
use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_transform::Transform;
use newengine_ui_api::{
    UiEditorTransformMode, UiEditorTransformSpace, UiEditorViewportProjection,
    UiEditorViewportState, UiInputFrame,
};

use super::types::EDITOR_HISTORY_LIMIT;
use super::{
    ActiveTransformDrag, EditorGizmoHandle, EditorViewportController, TransformTransaction,
};

impl EditorViewportController {
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
                sync_authored_map_placement_replicas(world, drag.entity);
                self.drag = Some(drag);
            }
        }
    }

    fn commit_drag(&mut self, world: &mut World, drag: ActiveTransformDrag) {
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
        let _ = world.insert(drag.entity, crate::gameplay::AuthoredMapPlacementDirty);
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
        sync_authored_map_placement_replicas(world, drag.entity);
        self.armed_handle = None;
    }

    fn undo(&mut self, world: &mut World) {
        let Some(transaction) = self.undo.pop() else {
            return;
        };
        if let Some(transform) = world.get_mut_tracked::<Transform>(transaction.entity) {
            *transform = transaction.before;
            self.redo.push(transaction);
            let _ = world.insert(
                transaction.entity,
                crate::gameplay::AuthoredMapPlacementDirty,
            );
            sync_authored_map_placement_replicas(world, transaction.entity);
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
            let _ = world.insert(
                transaction.entity,
                crate::gameplay::AuthoredMapPlacementDirty,
            );
            sync_authored_map_placement_replicas(world, transaction.entity);
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
}

#[inline]
fn min_vec3_component(value: Vec3) -> f32 {
    value.x.min(value.y).min(value.z)
}

#[inline]
fn max_abs_vec3_component(value: Vec3) -> f32 {
    value.x.abs().max(value.y.abs()).max(value.z.abs())
}

pub(crate) fn sync_authored_map_placement_replicas(world: &mut World, primary: EntityId) {
    use crate::gameplay::{
        AuthoredMapPlacement, AuthoredMapPlacementReplicaScaleState, AuthoredMapPlacementSource,
        StaticMeshCollider,
    };
    use newengine_bounds::Bounds;

    let Some(authored) = world.get::<AuthoredMapPlacement>(primary).cloned() else {
        return;
    };
    if !authored.primary || authored.source != AuthoredMapPlacementSource::DiscretePlacement {
        return;
    }
    let Some(primary_transform) = world.get::<Transform>(primary).copied() else {
        return;
    };
    if !primary_transform.position.is_finite()
        || !primary_transform.rotation.is_finite()
        || !primary_transform.scale.is_finite()
        || min_vec3_component(primary_transform.scale) <= 0.000_001
    {
        return;
    }

    let replicas = world
        .query::<AuthoredMapPlacement>()
        .filter_map(|(entity, candidate)| {
            (!candidate.primary
                && candidate.source == authored.source
                && candidate.map_ref == authored.map_ref
                && candidate.placement_id == authored.placement_id)
                .then_some(entity)
        })
        .collect::<Vec<_>>();

    for replica in replicas {
        if let Some(transform) = world.get_mut_tracked::<Transform>(replica) {
            transform.position = primary_transform.position;
            transform.rotation = primary_transform.rotation;
        }

        let Some(scale_state) = world
            .get::<AuthoredMapPlacementReplicaScaleState>(replica)
            .copied()
        else {
            continue;
        };
        let previous = scale_state.last_authored_scale;
        if !previous.is_finite() || min_vec3_component(previous) <= 0.000_001 {
            continue;
        }
        let ratio = Vec3::new(
            primary_transform.scale.x / previous.x,
            primary_transform.scale.y / previous.y,
            primary_transform.scale.z / previous.z,
        );
        if !ratio.is_finite() || min_vec3_component(ratio) <= 0.000_001 {
            continue;
        }
        if max_abs_vec3_component(ratio - Vec3::ONE) <= 1.0e-6 {
            continue;
        }

        let Some(collider) = world.get::<StaticMeshCollider>(replica).cloned() else {
            continue;
        };
        let vertices = collider
            .vertices
            .iter()
            .map(|vertex| {
                [
                    vertex[0] * ratio.x,
                    vertex[1] * ratio.y,
                    vertex[2] * ratio.z,
                ]
            })
            .collect::<Vec<_>>();
        let triangles = collider.triangles.as_ref().to_vec();
        let Ok(rescaled) = StaticMeshCollider::new(vertices, triangles)
            .map(|value| value.with_material(collider.friction, collider.restitution))
        else {
            continue;
        };
        let local_bounds = rescaled.local_bounds;
        let _ = world.insert(replica, rescaled);
        let _ = world.insert(replica, Bounds::from_local_aabb(local_bounds));
        let _ = world.insert(
            replica,
            AuthoredMapPlacementReplicaScaleState {
                last_authored_scale: primary_transform.scale,
            },
        );
    }
}

pub(super) fn transformed_from_drag(
    drag: ActiveTransformDrag,
    state: &UiEditorViewportState,
) -> Transform {
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

fn camera_plane_scale(
    camera: &CameraFrameSnapshot,
    object_position: Vec3,
    viewport_size: [u32; 2],
    projection: UiEditorViewportProjection,
    orthographic_half_height: f32,
) -> (Vec3, Vec3, f32) {
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
    (right, up, world_per_pixel)
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
    let (right, up, world_per_pixel) = camera_plane_scale(
        camera,
        object_position,
        viewport_size,
        projection,
        orthographic_half_height,
    );
    let screen_axis = [axis.dot(right), -axis.dot(up)];
    let screen_len = (screen_axis[0] * screen_axis[0] + screen_axis[1] * screen_axis[1]).sqrt();
    let projected_delta = if screen_len > 1.0e-4 {
        (mouse_delta.0 * screen_axis[0] + mouse_delta.1 * screen_axis[1]) / screen_len
    } else {
        mouse_delta.0 - mouse_delta.1
    };

    projected_delta * world_per_pixel
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
