use super::geometry::normalize_preview_geometry;
use super::*;

impl AssetPreviewApi {
    pub fn last_request_cache_hit(&self) -> bool {
        self.last_request_cache_hit.load(Ordering::Acquire)
    }

    pub fn invalidate(&self, asset_ref: &str) {
        let source = asset_ref.split('@').next().unwrap_or(asset_ref);
        self.bundle_cache.lock().retain(|entry| {
            entry
                .asset_ref
                .split('@')
                .next()
                .unwrap_or(&entry.asset_ref)
                != source
        });
        self.texture_cache
            .lock()
            .retain(|(cached_ref, _)| cached_ref.split('@').next().unwrap_or(cached_ref) != source);
    }

    pub fn invalidate_all(&self) {
        self.bundle_cache.lock().clear();
        self.texture_cache.lock().clear();
    }

    pub fn snapshot(&self) -> AssetPreviewSnapshot {
        let mut snapshot = self.current.lock().clone();
        if snapshot.kind == AssetPreviewKind::Scene3d {
            let texture_id = self.viewport.read_tex_user();
            snapshot.ui_texture_id = (texture_id != 0).then_some(texture_id);
            snapshot.ready = texture_id != 0;
            if snapshot.ready {
                snapshot.diagnostic = None;
            }
        }
        snapshot
    }

    pub fn clear(&self) {
        self.clear_render_bundle();
        *self.current.lock() = AssetPreviewSnapshot::unavailable("", "no asset selected");
    }

    pub(super) fn render_bundle(&self) -> Option<Arc<ModelAssetBundle>> {
        self.render_bundle.read().clone()
    }

    pub(super) fn activate_cached_bundle(&self, asset_ref: &str) -> bool {
        let mut cache = self.bundle_cache.lock();
        let Some(index) = cache.iter().position(|entry| entry.asset_ref == asset_ref) else {
            return false;
        };
        let entry = cache.remove(index).expect("cached bundle index must exist");
        let bundle = Arc::clone(&entry.bundle);
        cache.push_front(entry);
        drop(cache);
        self.camera.reset();
        *self.render_bundle.write() = Some(bundle);
        self.last_request_cache_hit.store(true, Ordering::Release);
        true
    }

    pub(super) fn set_render_bundle(&self, asset_ref: &str, mut bundle: ModelAssetBundle) {
        self.camera.reset();
        if let Some(summary) = normalize_preview_geometry(&mut bundle.parts) {
            newengine_ulog_api::ulog::info!(
                "asset preview: geometry normalized source='{}' source_center=({:.3},{:.3},{:.3}) source_extent=({:.3},{:.3},{:.3}) scale={:.6}",
                bundle.source,
                summary.source_center.x,
                summary.source_center.y,
                summary.source_center.z,
                summary.source_extent.x,
                summary.source_extent.y,
                summary.source_extent.z,
                summary.scale
            );
        }
        let vertex_count = bundle
            .parts
            .iter()
            .map(|part| part.mesh.vertices.len())
            .sum::<usize>();
        let bundle = Arc::new(bundle);
        *self.render_bundle.write() = Some(Arc::clone(&bundle));
        if vertex_count <= PREVIEW_BUNDLE_CACHE_MAX_VERTICES {
            let mut cache = self.bundle_cache.lock();
            cache.retain(|entry| entry.asset_ref != asset_ref);
            cache.push_front(CachedPreviewBundle {
                asset_ref: asset_ref.to_owned(),
                bundle,
            });
            cache.truncate(PREVIEW_BUNDLE_CACHE_CAPACITY);
        }
    }

    pub(super) fn cached_texture(&self, asset_ref: &str) -> Option<AssetPreviewSnapshot> {
        let mut cache = self.texture_cache.lock();
        let index = cache
            .iter()
            .position(|(cached_ref, _)| cached_ref == asset_ref)?;
        let entry = cache.remove(index)?;
        let snapshot = entry.1.clone();
        cache.push_front(entry);
        self.last_request_cache_hit.store(true, Ordering::Release);
        Some(snapshot)
    }

    pub(super) fn cache_texture(&self, snapshot: &AssetPreviewSnapshot) {
        let mut cache = self.texture_cache.lock();
        cache.retain(|(cached_ref, _)| cached_ref != &snapshot.asset_ref);
        cache.push_front((snapshot.asset_ref.clone(), snapshot.clone()));
        cache.truncate(PREVIEW_TEXTURE_CACHE_CAPACITY);
    }

    pub(super) fn clear_render_bundle(&self) {
        *self.render_bundle.write() = None;
        self.camera.reset();
        // Releasing a 3D bundle must return preview-only tools to their cheap
        // UI-only path. Without this, opening one model permanently keeps the
        // offscreen viewport and scene render path alive even after selecting a
        // 2D or non-visual asset.
        self.viewport.clear_external_extent();
    }
}
