use std::collections::BTreeMap;

use newengine_asset_format_nef8::ydd_binary::{YddBinarySkinVertex, YddBinaryVertex};
use newengine_math::Vec3;

use crate::pak::PakFile;

const PC_SUBMESH_STRIDE: usize = 192;
const LEGACY_NORTHSTAR_SUBMESH_STRIDE: usize = 176;
const STREAM_DESC_STRIDE: usize = 64;
const PACKED_WEIGHT_MASK: u32 = (1 << 22) - 1;
const PACKED_WEIGHT_DENOMINATOR: f32 = PACKED_WEIGHT_MASK as f32;

#[derive(Clone, Copy, Debug, Default)]
pub struct SkinLossStats {
    pub weighted_vertices: u64,
    pub source_influences: u64,
    pub max_source_influences: u32,
    pub top4_loss_sum: f64,
    pub top4_loss_max: f32,
    pub top8_loss_sum: f64,
    pub top8_loss_max: f32,
}

impl SkinLossStats {
    pub fn merge(&mut self, other: Self) {
        self.weighted_vertices += other.weighted_vertices;
        self.source_influences += other.source_influences;
        self.max_source_influences = self.max_source_influences.max(other.max_source_influences);
        self.top4_loss_sum += other.top4_loss_sum;
        self.top4_loss_max = self.top4_loss_max.max(other.top4_loss_max);
        self.top8_loss_sum += other.top8_loss_sum;
        self.top8_loss_max = self.top8_loss_max.max(other.top8_loss_max);
    }

    pub fn average_source_influences(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.source_influences as f64 / self.weighted_vertices as f64
        }
    }

    pub fn average_top4_loss(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.top4_loss_sum / self.weighted_vertices as f64
        }
    }

    pub fn average_top8_loss(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.top8_loss_sum / self.weighted_vertices as f64
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImportMesh {
    pub name: String,
    pub source_material: Option<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: Vec<YddBinaryVertex>,
    pub skin: Option<Vec<YddBinarySkinVertex>>,
    /// Size of the native skin joint domain declared by this submesh. For ordinary
    /// character geometry this matches the master JOINT_HIERARCHY size. Cloth-backed
    /// geometry can instead address a package-local simulation-node domain and must
    /// never be interpreted as master skeleton indices.
    pub source_skin_joint_domain_size: Option<usize>,
    pub skin_loss: SkinLossStats,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct DecodedGeometry {
    pub meshes: Vec<ImportMesh>,
    pub skin_loss: SkinLossStats,
}

#[derive(Clone, Debug)]
struct StreamDesc {
    kind: u8,
    buffer: usize,
    buffer_size: usize,
    num_vertices: usize,
    sizes: [u8; 4],
    q_scale: [f32; 4],
    q_offset: [f32; 4],
}

include!("geometry/decode.rs");
include!("geometry/streams.rs");
include!("geometry/compaction.rs");
include!("geometry/skin.rs");
include!("geometry/util.rs");

#[cfg(test)]
mod tests {
    include!("geometry/tests.rs");
}
