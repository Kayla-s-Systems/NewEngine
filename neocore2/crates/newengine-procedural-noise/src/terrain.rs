#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::{HeightField, NoiseGraph2D, TerrainHeightfieldDescriptor, TerrainHeightfieldSettings};

/// ECS-friendly render component for a generated heightfield.
#[derive(Clone, Debug)]
pub struct ProceduralTerrain {
    pub heightfield: Arc<HeightField>,
    pub base_color: [f32; 4],
}

impl ProceduralTerrain {
    #[inline]
    pub fn generate(settings: TerrainHeightfieldSettings, base_color: [f32; 4]) -> Self {
        Self {
            heightfield: Arc::new(HeightField::generate(settings)),
            base_color,
        }
    }

    #[inline]
    pub fn generate_descriptor(descriptor: TerrainHeightfieldDescriptor, base_color: [f32; 4]) -> Self {
        Self {
            heightfield: Arc::new(HeightField::generate_descriptor(descriptor)),
            base_color,
        }
    }

    #[inline]
    pub fn generate_with_graph(
        settings: TerrainHeightfieldSettings,
        graph: NoiseGraph2D,
        base_color: [f32; 4],
    ) -> Self {
        Self {
            heightfield: Arc::new(HeightField::generate_with_graph(settings, graph)),
            base_color,
        }
    }

    #[inline]
    pub fn mesh_key(&self) -> u64 {
        self.heightfield.revision_key()
    }
}
