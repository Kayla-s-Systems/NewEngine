#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CompiledAnimationGraphStoreKey {
    canonical_asset_path: String,
    skeleton_compatibility_key: u64,
    binding_variant_key: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompiledAnimationGraphStoreStats {
    pub compiled_graphs: usize,
    pub asset_paths: usize,
}

/// Process-shared immutable compiled graph cache.
///
/// The cache key includes both the compiled skeleton compatibility fingerprint and an optional
/// caller-supplied binding variant. The latter is required only when an authored graph resolves
/// symbolic clip aliases through product-owned bindings (for example one reusable humanoid graph
/// compiled against different character clip sets). Asset/clip I/O and graph compilation happen
/// outside the mutex; concurrent cold misses may compile redundantly, but insertion converges on
/// one authoritative `Arc<CompiledAnimationGraph>`.
#[derive(Debug, Default)]
pub struct CompiledAnimationGraphStore {
    graphs: Mutex<HashMap<CompiledAnimationGraphStoreKey, Arc<CompiledAnimationGraph>>>,
}

impl CompiledAnimationGraphStore {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn key(
        reference: &AnimationGraphAssetReference,
        skeleton: &AnimationSkeletonRuntime,
        binding_variant_key: u64,
    ) -> CompiledAnimationGraphStoreKey {
        CompiledAnimationGraphStoreKey {
            canonical_asset_path: reference.canonical_path_key.clone(),
            skeleton_compatibility_key: skeleton.compatibility_key(),
            binding_variant_key,
        }
    }

    pub fn load_or_compile<FAsset, FClip>(
        &self,
        reference: &str,
        skeleton: &AnimationSkeletonRuntime,
        load_asset: FAsset,
        load_clip: FClip,
    ) -> Result<Arc<CompiledAnimationGraph>, String>
    where
        FAsset: FnOnce(&str) -> Result<Vec<u8>, String>,
        FClip: FnMut(&str) -> Result<Arc<AnimationClip>, String>,
    {
        self.load_or_compile_with_variant(reference, skeleton, 0, load_asset, load_clip)
    }

    /// Variant-aware graph compilation for authored graphs whose clip references are symbolic and
    /// therefore depend on a product-owned binding table. `binding_variant_key` must change whenever
    /// that resolution contract changes. Direct asset->clip-reference graphs should use
    /// `load_or_compile`, which uses variant zero.
    pub fn load_or_compile_with_variant<FAsset, FClip>(
        &self,
        reference: &str,
        skeleton: &AnimationSkeletonRuntime,
        binding_variant_key: u64,
        load_asset: FAsset,
        mut load_clip: FClip,
    ) -> Result<Arc<CompiledAnimationGraph>, String>
    where
        FAsset: FnOnce(&str) -> Result<Vec<u8>, String>,
        FClip: FnMut(&str) -> Result<Arc<AnimationClip>, String>,
    {
        let reference = AnimationGraphAssetReference::parse(reference)?;
        let key = Self::key(&reference, skeleton, binding_variant_key);
        if let Some(cached) = self
            .graphs
            .lock()
            .map_err(|_| "compiled animation graph store mutex poisoned".to_owned())?
            .get(&key)
            .cloned()
        {
            return Ok(cached);
        }

        let bytes = load_asset(&reference.logical_path)?;
        let definition = decode_animation_graph_asset_v1(&bytes)?;
        let compiled = Arc::new(CompiledAnimationGraph::compile(
            definition,
            skeleton,
            |clip_ref| load_clip(clip_ref),
        )?);

        let mut guard = self
            .graphs
            .lock()
            .map_err(|_| "compiled animation graph store mutex poisoned".to_owned())?;
        Ok(guard.entry(key).or_insert_with(|| compiled.clone()).clone())
    }

    /// Invalidates every skeleton/variant-specialized compiled revision for an authored graph path.
    /// Already-bound actors keep their existing `Arc`; future bindings compile the new revision.
    pub fn invalidate_asset_path(&self, reference: &str) -> Result<usize, String> {
        let reference = AnimationGraphAssetReference::parse(reference)?;
        let mut guard = self
            .graphs
            .lock()
            .map_err(|_| "compiled animation graph store mutex poisoned".to_owned())?;
        let before = guard.len();
        guard.retain(|key, _| key.canonical_asset_path != reference.canonical_path_key);
        Ok(before - guard.len())
    }

    pub fn clear(&self) -> Result<(), String> {
        self.graphs
            .lock()
            .map(|mut guard| guard.clear())
            .map_err(|_| "compiled animation graph store mutex poisoned".to_owned())
    }

    pub fn stats(&self) -> Result<CompiledAnimationGraphStoreStats, String> {
        let guard = self
            .graphs
            .lock()
            .map_err(|_| "compiled animation graph store mutex poisoned".to_owned())?;
        let asset_paths = guard
            .keys()
            .map(|key| key.canonical_asset_path.as_str())
            .collect::<HashSet<_>>()
            .len();
        Ok(CompiledAnimationGraphStoreStats {
            compiled_graphs: guard.len(),
            asset_paths,
        })
    }
}

pub fn global_compiled_animation_graph_store() -> &'static CompiledAnimationGraphStore {
    static STORE: OnceLock<CompiledAnimationGraphStore> = OnceLock::new();
    STORE.get_or_init(CompiledAnimationGraphStore::new)
}
