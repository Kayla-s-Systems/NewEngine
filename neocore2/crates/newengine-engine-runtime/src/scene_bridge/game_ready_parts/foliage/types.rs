struct TreePlacement {
    index: u32,
    position: Vec3,
    yaw: f32,
    scale: f32,
}


struct RuntimePrefabMeshPart {
    primitive_id: PrimitiveId,
    material_slot: String,
    material_ref: Option<String>,
    material_id: MaterialId,
    color: [f32; 4],
}

#[derive(Clone, Debug)]
struct DecodedPrefabMeshPart {
    primitive_id: PrimitiveId,
    name: String,
    material_slot: String,
    material_ref: Option<String>,
    mesh: PrimitiveMesh,
}


const SKYDOME_PRIMITIVE_ID: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.asset.skydome.high.v1"));
