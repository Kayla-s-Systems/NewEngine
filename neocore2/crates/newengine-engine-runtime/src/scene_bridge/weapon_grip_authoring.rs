use newengine_assets_api::{
    asset_edit_method, definitions_method, AssetPatch, AssetPatchOperation, AssetPatchResult,
    ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, ENGINE_ASSETS_EDIT_SERVICE_ID,
};
use newengine_math::{Quat, Vec3};
use serde_json::{json, Value};

use super::SceneBridge;
use crate::gameplay::{
    active_equipped_weapon_component_modifiers, ItemCatalog, WeaponEntityRuntime,
};

const WEAPON_YTYP_EDIT_CONTRACT: &str = "asset.edit.ytyp.v1";
const GRIP_WRITEBACK_EPSILON_SQ: f32 = 1.0e-10;

#[derive(Clone, Debug)]
struct WeaponGripWriteback {
    definition_ref: String,
    handle_from_root: Vec3,
    ready_body_to_root_rotation: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BakedWeaponGrip {
    handle_from_root: Vec3,
    ready_body_to_root_rotation: Quat,
}

impl SceneBridge {
    /// Persist a selected held-weapon gizmo correction into its authored YTYP grip contract.
    ///
    /// RuntimeTransformEditOverride intentionally remains a transient editor layer. Save converts
    /// that world-space delta into the weapon's authored local grip frame and writes the existing
    /// `handle_from_root` + `ready_body_to_root_rotation` fields through engine.assets.edit.
    pub(super) fn save_selected_weapon_grip_asset(&self) -> Result<bool, String> {
        let Some(writeback) = self.selected_weapon_grip_writeback()? else {
            return Ok(false);
        };
        apply_weapon_grip_writeback(&writeback)?;
        newengine_ulog_api::ulog::info!(
            "editor weapon grip: authored YTYP writeback complete ref='{}' handle_from_root={:?} ready_body_to_root_rotation={:?}",
            writeback.definition_ref,
            [writeback.handle_from_root.x, writeback.handle_from_root.y, writeback.handle_from_root.z],
            [
                writeback.ready_body_to_root_rotation.x,
                writeback.ready_body_to_root_rotation.y,
                writeback.ready_body_to_root_rotation.z,
                writeback.ready_body_to_root_rotation.w,
            ],
        );
        Ok(true)
    }

    fn selected_weapon_grip_writeback(&self) -> Result<Option<WeaponGripWriteback>, String> {
        let Some(selected) = self.selection() else {
            return Ok(None);
        };
        let scene = self.scene.read();
        let world = scene.world();
        let Some(runtime) = world.get::<WeaponEntityRuntime>(selected).copied() else {
            return Ok(None);
        };
        let Some(manual) = world
            .get::<newengine_transform_api::RuntimeTransformEditOverride>(selected)
            .copied()
        else {
            return Ok(None);
        };
        if !has_manual_grip_delta(manual.position_offset(), manual.rotation_offset()) {
            return Ok(None);
        }
        let definition = world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(runtime.item))
            .ok_or_else(|| {
                format!(
                    "editor weapon grip: item definition missing item={:016x}",
                    runtime.item.0
                )
            })?;
        let definition_ref = definition
            .definition_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "editor weapon grip: item '{}' has no authored definition_ref; runtime-only weapons cannot be written back",
                    definition.name
                )
            })?
            .to_owned();
        if !definition.weapon_presentation.enabled {
            return Err(format!(
                "editor weapon grip: item '{}' has no enabled authored weapon presentation",
                definition.name
            ));
        }
        let component_offset = active_equipped_weapon_component_modifiers(world, runtime.owner)
            .presentation_offset_local;
        let baked = bake_weapon_grip(
            v3(definition.weapon_presentation.handle_from_root),
            q4(definition.weapon_presentation.ready_body_to_root_rotation),
            q4(definition.weapon_presentation.native_rig_to_runtime_basis),
            v3(component_offset),
            manual.base().rotation,
            manual.position_offset(),
            manual.rotation_offset(),
        )?;
        Ok(Some(WeaponGripWriteback {
            definition_ref,
            handle_from_root: baked.handle_from_root,
            ready_body_to_root_rotation: baked.ready_body_to_root_rotation,
        }))
    }
}

fn has_manual_grip_delta(position_offset: Vec3, rotation_offset: Quat) -> bool {
    if !position_offset.is_finite() || !rotation_offset.is_finite() {
        return false;
    }
    let rotation = rotation_offset.normalize_or_identity();
    position_offset.length_squared() > GRIP_WRITEBACK_EPSILON_SQ
        || rotation.dot(Quat::IDENTITY).abs() < 0.999_999_5
}

fn bake_weapon_grip(
    handle_from_root: Vec3,
    ready_body_to_root_rotation: Quat,
    native_rig_to_runtime_basis: Quat,
    component_offset_local: Vec3,
    runtime_base_rotation: Quat,
    runtime_position_offset_world: Vec3,
    runtime_rotation_offset_local: Quat,
) -> Result<BakedWeaponGrip, String> {
    let values_are_finite = handle_from_root.is_finite()
        && ready_body_to_root_rotation.is_finite()
        && native_rig_to_runtime_basis.is_finite()
        && component_offset_local.is_finite()
        && runtime_base_rotation.is_finite()
        && runtime_position_offset_world.is_finite()
        && runtime_rotation_offset_local.is_finite();
    if !values_are_finite {
        return Err("editor weapon grip: non-finite transform cannot be authored".to_owned());
    }

    let base_rotation = runtime_base_rotation.normalize_or_identity();
    let delta_rotation = runtime_rotation_offset_local.normalize_or_identity();
    let native_basis = native_rig_to_runtime_basis.normalize_or_identity();
    let ready_rotation = ready_body_to_root_rotation.normalize_or_identity();
    let local_translation_delta = base_rotation.inverse() * runtime_position_offset_world;

    // The ordinary grip solve is root = anchor - R*handle + R*component_offset. Re-author both
    // translation and rotation so the same final root is reconstructed with no runtime override.
    // Keeping component offset in the equation prevents installed weapon parts from contaminating
    // the base grip calibration.
    let handle_from_root = delta_rotation.inverse()
        * (handle_from_root - component_offset_local - local_translation_delta)
        + component_offset_local;

    // Runtime root orientation is body * ready * native_basis. The gizmo delta is post-multiplied
    // on that root, therefore fold it into `ready` by conjugating through the native basis.
    let ready_body_to_root_rotation =
        (ready_rotation * native_basis * delta_rotation * native_basis.inverse())
            .normalize_or_identity();

    if !handle_from_root.is_finite() || !ready_body_to_root_rotation.is_finite() {
        return Err("editor weapon grip: baked authored transform is non-finite".to_owned());
    }
    if handle_from_root.length() > 10.0 {
        return Err(format!(
            "editor weapon grip: baked handle_from_root is implausible ({:.3}m); refusing destructive writeback",
            handle_from_root.length()
        ));
    }
    Ok(BakedWeaponGrip {
        handle_from_root,
        ready_body_to_root_rotation,
    })
}

fn apply_weapon_grip_writeback(writeback: &WeaponGripWriteback) -> Result<(), String> {
    let entry = load_definition_entry(&writeback.definition_ref)?;
    let identity = entry
        .get("identity")
        .and_then(Value::as_object)
        .ok_or_else(|| "editor weapon grip: definitions response has no identity".to_owned())?;
    let source = identity
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "editor weapon grip: definitions response has no source path".to_owned())?;
    let entry_name = identity
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "editor weapon grip: definitions response has no entry name".to_owned())?;

    let mut raw_metadata = entry
        .pointer("/arbitrary_metadata/metadata")
        .cloned()
        .filter(Value::is_object)
        .ok_or_else(|| {
            "editor weapon grip: YTYP definition has no writable arbitrary_metadata.metadata object"
                .to_owned()
        })?;
    patch_weapon_presentation_metadata(
        &mut raw_metadata,
        writeback.handle_from_root,
        writeback.ready_body_to_root_rotation,
    )?;

    let target_ref = format!("{source}@{entry_name}");
    let patch = AssetPatch {
        asset_ref: target_ref.clone(),
        provider_service: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID.to_owned(),
        edit_contract: WEAPON_YTYP_EDIT_CONTRACT.to_owned(),
        operations: vec![AssetPatchOperation {
            op: "replace".to_owned(),
            path: "/metadata".to_owned(),
            value: json!({ "metadata": raw_metadata }),
            ..AssetPatchOperation::default()
        }],
        requester: "engine.editing.tools.weapon_grip_gizmo".to_owned(),
        ..AssetPatch::default()
    };
    let payload = serde_json::to_vec(&patch)
        .map_err(|error| format!("editor weapon grip: patch encode failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        ENGINE_ASSETS_EDIT_SERVICE_ID,
        asset_edit_method::APPLY_PATCH_JSON_V1,
        &payload,
    )
    .map_err(|error| format!("editor weapon grip: engine.assets.edit failed: {error}"))?
    .ok_or_else(|| "editor weapon grip: engine.assets.edit route is unavailable".to_owned())?;
    let result: AssetPatchResult = serde_json::from_slice(&bytes)
        .map_err(|error| format!("editor weapon grip: invalid edit response: {error}"))?;
    if result.accepted && result.written {
        return Ok(());
    }
    let details = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .filter(|message| !message.trim().is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    Err(if details.is_empty() {
        format!(
            "editor weapon grip: YTYP writeback was not committed target='{}' accepted={} written={} dirty={}",
            target_ref, result.accepted, result.written, result.dirty
        )
    } else {
        format!(
            "editor weapon grip: YTYP writeback was not committed target='{}': {}",
            target_ref, details
        )
    })
}

fn load_definition_entry(definition_ref: &str) -> Result<Value, String> {
    let payload =
        serde_json::to_vec(&json!({ "definition_ref": definition_ref })).map_err(|error| {
            format!("editor weapon grip: definition request encode failed: {error}")
        })?;
    let bytes = newengine_core::call_service_v1_optional(
        ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        definitions_method::ENTRY_JSON_V1,
        &payload,
    )
    .map_err(|error| format!("editor weapon grip: definition lookup failed: {error}"))?
    .ok_or_else(|| "editor weapon grip: definitions route is unavailable".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("editor weapon grip: invalid definition response: {error}"))
}

fn patch_weapon_presentation_metadata(
    metadata: &mut Value,
    handle_from_root: Vec3,
    ready_body_to_root_rotation: Quat,
) -> Result<(), String> {
    let metadata = metadata
        .as_object_mut()
        .ok_or_else(|| "editor weapon grip: metadata payload is not an object".to_owned())?;
    let weapon = metadata
        .get_mut("newengine.weapon")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "editor weapon grip: newengine.weapon metadata is missing".to_owned())?;
    let presentation = weapon
        .entry("presentation".to_owned())
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| {
            "editor weapon grip: newengine.weapon.presentation is not an object".to_owned()
        })?;
    presentation.insert(
        "handle_from_root".to_owned(),
        json!([handle_from_root.x, handle_from_root.y, handle_from_root.z]),
    );
    presentation.insert(
        "ready_body_to_root_rotation".to_owned(),
        json!([
            ready_body_to_root_rotation.x,
            ready_body_to_root_rotation.y,
            ready_body_to_root_rotation.z,
            ready_body_to_root_rotation.w,
        ]),
    );
    Ok(())
}

#[inline]
fn v3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn q4(value: [f32; 4]) -> Quat {
    Quat::from_xyzw(value[0], value[1], value[2], value[3]).normalize_or_identity()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_gizmo_bakes_into_handle_from_root() {
        let baked = bake_weapon_grip(
            Vec3::new(0.0, 0.0, 0.4),
            Quat::IDENTITY,
            Quat::IDENTITY,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(0.1, -0.2, 0.05),
            Quat::IDENTITY,
        )
        .unwrap();
        assert!((baked.handle_from_root - Vec3::new(-0.1, 0.2, 0.35)).length() < 1.0e-6);
        assert!(baked.ready_body_to_root_rotation.dot(Quat::IDENTITY).abs() > 0.999_999);
    }

    #[test]
    fn rotation_gizmo_bakes_through_native_basis_without_losing_handle_anchor() {
        let handle = Vec3::new(0.05, -0.02, 0.33);
        let native = Quat::from_rotation_x(core::f32::consts::FRAC_PI_2);
        let delta = Quat::from_rotation_y(0.25);
        let baked = bake_weapon_grip(
            handle,
            Quat::IDENTITY,
            native,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            delta,
        )
        .unwrap();
        let expected_ready = (native * delta * native.inverse()).normalize_or_identity();
        assert!(baked.ready_body_to_root_rotation.dot(expected_ready).abs() > 0.999_999);
        assert!((delta * baked.handle_from_root - handle).length() < 1.0e-6);
    }

    #[test]
    fn metadata_patch_preserves_unrelated_weapon_and_asset_metadata() {
        let mut metadata = json!({
            "newengine.weapon": {
                "weapon": { "damage": 42.0 },
                "presentation": { "muzzle_from_root": [0.0, 0.0, 0.7] }
            },
            "newengine.render": { "cast_shadows": true }
        });
        patch_weapon_presentation_metadata(
            &mut metadata,
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_rotation_z(0.4),
        )
        .unwrap();
        assert_eq!(
            metadata["newengine.weapon"]["weapon"]["damage"],
            json!(42.0)
        );
        assert_eq!(
            metadata["newengine.weapon"]["presentation"]["muzzle_from_root"],
            json!([0.0, 0.0, 0.7])
        );
        assert_eq!(metadata["newengine.render"]["cast_shadows"], json!(true));
        assert_eq!(
            metadata["newengine.weapon"]["presentation"]["handle_from_root"],
            json!([1.0, 2.0, 3.0])
        );
    }
}
