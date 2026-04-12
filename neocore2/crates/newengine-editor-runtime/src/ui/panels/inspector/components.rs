#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use crate::scene_bridge::{
    SceneImportedAssetKind, SceneImportedAssetRepresentation,
};

#[inline]
pub(crate) fn dir_to_yaw_pitch_deg(direction: newengine_math::Vec3) -> (f32, f32) {
    let normalized = direction.normalize_or_zero();
    let yaw = normalized.x.atan2(normalized.z);
    let pitch = (-normalized.y).asin();
    (yaw.to_degrees(), pitch.to_degrees())
}

#[inline]
pub(crate) fn yaw_pitch_deg_to_dir(yaw_deg: f32, pitch_deg: f32) -> newengine_math::Vec3 {
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();
    let cy = yaw.cos();
    let sy = yaw.sin();
    let cp = pitch.cos();
    let sp = pitch.sin();
    newengine_math::Vec3::new(sy * cp, -sp, cy * cp).normalize_or_zero()
}

#[inline]
pub(crate) fn imported_kind_label(kind: SceneImportedAssetKind) -> &'static str {
    match kind {
        SceneImportedAssetKind::StaticMesh => "Static Mesh",
        SceneImportedAssetKind::SceneReference => "Scene Reference",
        SceneImportedAssetKind::TextureReference => "Texture Reference",
        SceneImportedAssetKind::MaterialReference => "Material Reference",
        SceneImportedAssetKind::OpaqueReference => "Opaque Reference",
    }
}

#[inline]
pub(crate) fn imported_repr_label(kind: SceneImportedAssetRepresentation) -> &'static str {
    match kind {
        SceneImportedAssetRepresentation::PrimitiveCube => "Primitive Cube",
        SceneImportedAssetRepresentation::PrimitivePlane => "Primitive Plane",
        SceneImportedAssetRepresentation::PrimitiveSphere => "Primitive Sphere",
    }
}

#[inline]
pub(crate) fn drag_triplet(
    ui: &mut egui::Ui,
    values: &mut [f32; 3],
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut values[0]).speed(speed).range(range.clone()));
        ui.add(egui::DragValue::new(&mut values[1]).speed(speed).range(range.clone()));
        ui.add(egui::DragValue::new(&mut values[2]).speed(speed).range(range));
    });
}
