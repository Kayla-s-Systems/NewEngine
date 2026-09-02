use newengine_materials::{
    AuthoredMaterialLibrary, MaterialDescriptorLoadResponse, MaterialTextureRefInfo,
    ResolvedMaterialGraph,
};
use newengine_math::collections::BoundedCache;
use std::sync::{Arc, Mutex, OnceLock};

const MATERIAL_LIBRARY_CACHE_CAPACITY: usize = 16;
const MATERIAL_DESCRIPTOR_CACHE_CAPACITY: usize = 512;
const MATERIAL_GRAPH_CACHE_CAPACITY: usize = 128;
const MATERIAL_TEXTURE_REF_CACHE_CAPACITY: usize = 512;

pub(crate) struct MaterialRuntimeCaches {
    pub(crate) libraries: BoundedCache<String, Arc<AuthoredMaterialLibrary>>,
    pub(crate) descriptors: BoundedCache<String, MaterialDescriptorLoadResponse>,
    pub(crate) graphs: BoundedCache<String, ResolvedMaterialGraph>,
    pub(crate) texture_refs: BoundedCache<String, MaterialTextureRefInfo>,
}

impl Default for MaterialRuntimeCaches {
    fn default() -> Self {
        Self {
            libraries: BoundedCache::new(MATERIAL_LIBRARY_CACHE_CAPACITY),
            descriptors: BoundedCache::new(MATERIAL_DESCRIPTOR_CACHE_CAPACITY),
            graphs: BoundedCache::new(MATERIAL_GRAPH_CACHE_CAPACITY),
            texture_refs: BoundedCache::new(MATERIAL_TEXTURE_REF_CACHE_CAPACITY),
        }
    }
}

pub(crate) fn shared_material_runtime_caches() -> Arc<Mutex<MaterialRuntimeCaches>> {
    static CACHES: OnceLock<Arc<Mutex<MaterialRuntimeCaches>>> = OnceLock::new();
    Arc::clone(CACHES.get_or_init(|| Arc::new(Mutex::new(MaterialRuntimeCaches::default()))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_runtime_caches_are_bounded() {
        let caches = MaterialRuntimeCaches::default();
        assert_eq!(caches.libraries.capacity(), MATERIAL_LIBRARY_CACHE_CAPACITY);
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
