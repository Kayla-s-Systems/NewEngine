use newengine_materials::{
    MaterialDescriptorLoadResponse, MaterialTextureRefInfo, ResolvedMaterialGraph,
};
use newengine_math::collections::BoundedCache;

const MATERIAL_DESCRIPTOR_CACHE_CAPACITY: usize = 128;
const MATERIAL_GRAPH_CACHE_CAPACITY: usize = 128;
const MATERIAL_TEXTURE_REF_CACHE_CAPACITY: usize = 512;

pub(crate) struct MaterialRuntimeCaches {
    pub(crate) descriptors: BoundedCache<String, MaterialDescriptorLoadResponse>,
    pub(crate) graphs: BoundedCache<String, ResolvedMaterialGraph>,
    pub(crate) texture_refs: BoundedCache<String, MaterialTextureRefInfo>,
}

impl Default for MaterialRuntimeCaches {
    fn default() -> Self {
        Self {
            descriptors: BoundedCache::new(MATERIAL_DESCRIPTOR_CACHE_CAPACITY),
            graphs: BoundedCache::new(MATERIAL_GRAPH_CACHE_CAPACITY),
            texture_refs: BoundedCache::new(MATERIAL_TEXTURE_REF_CACHE_CAPACITY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_runtime_caches_are_bounded() {
        let caches = MaterialRuntimeCaches::default();
        assert_eq!(
            caches.descriptors.capacity(),
            MATERIAL_DESCRIPTOR_CACHE_CAPACITY
        );
        assert_eq!(caches.graphs.capacity(), MATERIAL_GRAPH_CACHE_CAPACITY);
        assert_eq!(
            caches.texture_refs.capacity(),
            MATERIAL_TEXTURE_REF_CACHE_CAPACITY
        );
    }
}
