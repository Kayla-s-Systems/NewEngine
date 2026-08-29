use super::provider::AssetPreviewDrawListProvider;
use super::*;

impl AssetPreviewApi {
    pub fn new(viewport: Arc<ViewportBridge>) -> Self {
        let host = newengine_plugin_host::default_host_api();
        let assets = AssetServiceClient::new(host.clone());
        Self {
            models: ModelGatewayClient::new(host.clone()),
            materials: MaterialGatewayClient::new(host.clone()),
            host,
            assets,
            viewport,
            current: Mutex::new(AssetPreviewSnapshot::unavailable("", "no asset selected")),
            render_bundle: RwLock::new(None),
            bundle_cache: Mutex::new(BoundedCache::new(PREVIEW_BUNDLE_CACHE_CAPACITY)),
            texture_cache: Mutex::new(BoundedCache::new(PREVIEW_TEXTURE_CACHE_CAPACITY)),
            last_request_cache_hit: AtomicBool::new(false),
            camera: AssetPreviewCameraState::default(),
        }
    }

    pub fn draw_list_provider(self: &Arc<Self>) -> Arc<dyn RenderDrawListProvider> {
        Arc::new(AssetPreviewDrawListProvider {
            api: Arc::clone(self),
        })
    }
}
