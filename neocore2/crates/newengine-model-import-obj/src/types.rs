use std::collections::BTreeMap;

use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMaterialSource {
    pub kd: [f32; 3],
    pub alpha: f32,
    pub ns: f32,
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

impl Default for ModelMaterialSource {
    #[inline]
    fn default() -> Self {
        Self {
            kd: [0.82, 0.78, 0.72],
            alpha: 1.0,
            ns: 32.0,
            base_color_texture: None,
            normal_texture: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObjPart {
    pub material_slot: String,
    pub mesh: PrimitiveMesh,
}

#[derive(Clone, Debug)]
pub struct ObjDecodeResult {
    pub parts: Vec<ObjPart>,
    pub materials: BTreeMap<String, ModelMaterialSource>,
    pub mtllibs: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ObjCorner {
    pub(crate) pos: usize,
    pub(crate) uv: Option<usize>,
    pub(crate) nrm: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ObjPartBuilder {
    pub(crate) vertices: Vec<PrimitiveVertex>,
    pub(crate) indices: Vec<u32>,
}
