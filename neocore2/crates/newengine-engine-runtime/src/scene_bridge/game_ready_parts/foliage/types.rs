use super::*;

pub(in crate::scene_bridge::game_ready) struct TreePlacement {
    pub(in crate::scene_bridge::game_ready) index: u32,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) yaw: f32,
    pub(in crate::scene_bridge::game_ready) scale: f32,
}

pub(in crate::scene_bridge::game_ready) struct RuntimePrefabMeshPart {
    pub(in crate::scene_bridge::game_ready) primitive_id: PrimitiveId,
    pub(in crate::scene_bridge::game_ready) material_slot: String,
    pub(in crate::scene_bridge::game_ready) material_id: MaterialId,
    pub(in crate::scene_bridge::game_ready) color: [f32; 4],
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct DecodedPrefabMeshPart {
    pub(in crate::scene_bridge::game_ready) primitive_id: PrimitiveId,
    pub(in crate::scene_bridge::game_ready) name: String,
    pub(in crate::scene_bridge::game_ready) material_slot: String,
    pub(in crate::scene_bridge::game_ready) material_ref: Option<String>,
    pub(in crate::scene_bridge::game_ready) mesh: PrimitiveMesh,
}

pub(in crate::scene_bridge::game_ready) const SKYDOME_PRIMITIVE_ID: PrimitiveId =
    PrimitiveId(fnv1a_64("kalitech.asset.skydome.high.v1"));
