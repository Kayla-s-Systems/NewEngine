use super::*;

pub(super) fn publish_inspector_snapshot_to_surface(
    snapshot: &serde_json::Value,
    surface_id: &str,
) {
    let Some(object) = snapshot.as_object() else {
        return;
    };
    let mut patch = UiStatePatch::new(0, surface_id);
    for (key, value) in object {
        patch = patch.with_change("selected_entity", key.clone(), value.clone());
    }
    crate::ui_gateway::publish_state_patch(&patch, "engine.scene", INSPECTOR_CONTRACT);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransformEditField {
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
}

impl TransformEditField {
    pub(super) fn parse(action_id: &str) -> Option<Self> {
        let suffix = action_id
            .trim()
            .strip_prefix(IN_GAME_EDITOR_TRANSFORM_PREFIX)?;
        Some(match suffix {
            "position.x" => Self::PositionX,
            "position.y" => Self::PositionY,
            "position.z" => Self::PositionZ,
            "rotation.x" => Self::RotationX,
            "rotation.y" => Self::RotationY,
            "rotation.z" => Self::RotationZ,
            "scale.x" => Self::ScaleX,
            "scale.y" => Self::ScaleY,
            "scale.z" => Self::ScaleZ,
            _ => return None,
        })
    }

    pub(super) fn apply(self, transform: &mut Transform, value: f32) -> bool {
        let value = value.clamp(-1_000_000.0, 1_000_000.0);
        match self {
            Self::PositionX => transform.position.x = value,
            Self::PositionY => transform.position.y = value,
            Self::PositionZ => transform.position.z = value,
            Self::ScaleX => transform.scale.x = sanitize_scale(value),
            Self::ScaleY => transform.scale.y = sanitize_scale(value),
            Self::ScaleZ => transform.scale.z = sanitize_scale(value),
            Self::RotationX | Self::RotationY | Self::RotationZ => {
                let (mut yaw, mut pitch, mut roll) = transform.yaw_pitch_roll();
                match self {
                    Self::RotationX => pitch = value.to_radians(),
                    Self::RotationY => yaw = value.to_radians(),
                    Self::RotationZ => roll = value.to_radians(),
                    _ => unreachable!(),
                }
                transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
            }
        }
        true
    }
}

#[inline]
fn sanitize_scale(value: f32) -> f32 {
    if value.abs() < 0.001 {
        0.001_f32.copysign(if value == 0.0 { 1.0 } else { value })
    } else {
        value.clamp(-10_000.0, 10_000.0)
    }
}

pub(super) fn action_payload_f32(payload: &serde_json::Value) -> Option<f32> {
    let value = payload.get("value").unwrap_or(payload);
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| value.as_str()?.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

pub(super) struct TransformScalarFields {
    pub(super) position_x: String,
    pub(super) position_y: String,
    pub(super) position_z: String,
    pub(super) rotation_x: String,
    pub(super) rotation_y: String,
    pub(super) rotation_z: String,
    pub(super) scale_x: String,
    pub(super) scale_y: String,
    pub(super) scale_z: String,
}

impl TransformScalarFields {
    pub(super) fn identity() -> Self {
        Self {
            position_x: "0.000".to_owned(),
            position_y: "0.000".to_owned(),
            position_z: "0.000".to_owned(),
            rotation_x: "0.000".to_owned(),
            rotation_y: "0.000".to_owned(),
            rotation_z: "0.000".to_owned(),
            scale_x: "1.000".to_owned(),
            scale_y: "1.000".to_owned(),
            scale_z: "1.000".to_owned(),
        }
    }
}

pub(super) fn transform_scalar_fields(transform: Transform) -> TransformScalarFields {
    let (yaw, pitch, roll) = transform.yaw_pitch_roll();
    TransformScalarFields {
        position_x: format!("{:.3}", transform.position.x),
        position_y: format!("{:.3}", transform.position.y),
        position_z: format!("{:.3}", transform.position.z),
        rotation_x: format!("{:.3}", pitch.to_degrees()),
        rotation_y: format!("{:.3}", yaw.to_degrees()),
        rotation_z: format!("{:.3}", roll.to_degrees()),
        scale_x: format!("{:.3}", transform.scale.x),
        scale_y: format!("{:.3}", transform.scale.y),
        scale_z: format!("{:.3}", transform.scale.z),
    }
}

pub(super) fn transform_snapshot_json(transform: &Transform) -> serde_json::Value {
    let (yaw, pitch, roll) = transform.yaw_pitch_roll();
    serde_json::json!({
        "position": [transform.position.x, transform.position.y, transform.position.z],
        "rotation": format!("{:?}", transform.rotation),
        "rotation_degrees_xyz": [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()],
        "scale": [transform.scale.x, transform.scale.y, transform.scale.z],
    })
}

pub(super) fn bounds_summary(bounds: Bounds) -> String {
    let center = bounds.world_aabb.center();
    let half = bounds.world_aabb.half_extents();
    format!(
        "{:?} center=({:.2}, {:.2}, {:.2}) half=({:.2}, {:.2}, {:.2})",
        bounds.kind, center.x, center.y, center.z, half.x, half.y, half.z
    )
}

pub(super) fn physics_summary(body: crate::gameplay::PhysicsBodyDesc) -> String {
    format!(
        "{:?} / {:?} trigger={} query={} friction={:.2}",
        body.kind,
        body.shape,
        body.flags.is_trigger,
        body.flags.participates_in_queries,
        body.material.friction,
    )
}

pub(super) fn anchor_summary(anchor: &crate::gameplay::SceneEntityAnchor) -> String {
    format!("{:?}: {}", anchor.role, anchor.label)
}

pub(super) fn bounds_snapshot_json(bounds: &Bounds) -> serde_json::Value {
    let local_center = bounds.local_aabb.center();
    let local_half = bounds.local_aabb.half_extents();
    let world_center = bounds.world_aabb.center();
    let world_half = bounds.world_aabb.half_extents();
    serde_json::json!({
        "kind": format!("{:?}", bounds.kind),
        "local_center": [local_center.x, local_center.y, local_center.z],
        "local_half_extents": [local_half.x, local_half.y, local_half.z],
        "world_center": [world_center.x, world_center.y, world_center.z],
        "world_half_extents": [world_half.x, world_half.y, world_half.z],
    })
}

pub(super) fn physics_body_snapshot_json(
    body: &crate::gameplay::PhysicsBodyDesc,
) -> serde_json::Value {
    serde_json::json!({
        "kind": format!("{:?}", body.kind),
        "shape": format!("{:?}", body.shape),
        "is_trigger": body.flags.is_trigger,
        "participates_in_queries": body.flags.participates_in_queries,
        "casts_contacts": body.flags.casts_contacts,
        "material": {
            "friction": body.material.friction,
            "restitution": body.material.restitution,
            "density": body.material.density,
        },
    })
}

pub(super) fn scene_anchor_snapshot_json(
    anchor: &crate::gameplay::SceneEntityAnchor,
) -> serde_json::Value {
    serde_json::json!({
        "role": format!("{:?}", anchor.role),
        "label": anchor.label,
    })
}

pub(super) fn selection_entity_key_from_action(
    action_id: &str,
    payload: &serde_json::Value,
) -> Option<u64> {
    const PREFIX: &str = "engine.editor.selection.select_entity";
    let action_id = action_id.trim();
    if let Some(suffix) = action_id.strip_prefix(&(PREFIX.to_owned() + ".")) {
        if let Ok(value) = suffix.trim().parse::<u64>() {
            return Some(value);
        }
    }
    if action_id != PREFIX {
        return None;
    }
    payload
        .get("entity_key")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            payload
                .get("entity")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
}
