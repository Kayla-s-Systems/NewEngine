#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use abi_stable::std_types::RString;
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_assets_api::{
    AssetDocument, AssetEntryManifest, AssetFileManifest, ASSET_LIST_FILE_MANIFEST_OUTPUT,
};
use newengine_material_client::MaterialGatewayClient;
use newengine_materials::MaterialLoadRequest;
use newengine_math::{collections::BoundedCache, Vec3};
use newengine_model_client::ModelGatewayClient;
use newengine_model_domain_api::{
    AssetGraphResolveRequest, ModelAssetBundle, ModelAssetRequest, ModelMaterialBinding,
    ModelMeshPart, ModelRuntimeConfiguration, ResolvedAssetGraphV2, ASSET_GRAPH_METHOD_RESOLVE_V1,
    ENGINE_ASSETS_GRAPH_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_primitives::{builtins, PrimitiveRegistry};
use newengine_render_feature_api::{
    opaque_list, AssetPreviewView, DrawListBuildCtx, RenderDrawListProvider, SceneExtractionCtx,
};
use parking_lot::{Mutex, RwLock};

use newengine_viewport_bridge::ViewportBridge;

const PREVIEW_ORBIT_SENSITIVITY: f32 = 0.008;
const PREVIEW_PAN_SENSITIVITY: f32 = 0.0015;
const PREVIEW_MAX_TARGET_OFFSET: f32 = 8.0;
const PREVIEW_MIN_PITCH: f32 = -1.30;
const PREVIEW_MAX_PITCH: f32 = 1.30;
const PREVIEW_MIN_DISTANCE: f32 = 1.65;
const PREVIEW_MAX_DISTANCE: f32 = 12.0;
const PREVIEW_BUNDLE_CACHE_CAPACITY: usize = 3;
const PREVIEW_BUNDLE_CACHE_MAX_VERTICES: usize = 500_000;
const PREVIEW_TEXTURE_CACHE_CAPACITY: usize = 12;

mod api;
mod camera;
mod geometry;
mod provider;
mod request;
mod state;
#[cfg(test)]
mod tests;

use camera::AssetPreviewCameraState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetPreviewKind {
    None,
    Texture2d,
    Scene3d,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetPreviewSnapshot {
    pub asset_ref: String,
    pub kind: AssetPreviewKind,
    pub ready: bool,
    pub texture_ref: Option<String>,
    pub ui_texture_id: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub diagnostic: Option<String>,
}

impl AssetPreviewSnapshot {
    fn unavailable(asset_ref: impl Into<String>, diagnostic: impl Into<String>) -> Self {
        Self {
            asset_ref: asset_ref.into(),
            kind: AssetPreviewKind::None,
            ready: false,
            texture_ref: None,
            ui_texture_id: None,
            width: 0,
            height: 0,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

/// Single engine API point for visual asset previews.
///
/// It resolves assets through the same definitions/graph/model/material/texture
/// gateways as game runtime. 3D data is stored as a render-only ModelAssetBundle;
/// no ECS entities, gameplay state or physics objects are created.
pub struct AssetPreviewApi {
    host: HostApiV1,
    assets: AssetServiceClient,
    models: ModelGatewayClient,
    materials: MaterialGatewayClient,
    viewport: Arc<ViewportBridge>,
    current: Mutex<AssetPreviewSnapshot>,
    render_bundle: RwLock<Option<Arc<ModelAssetBundle>>>,
    bundle_cache: Mutex<BoundedCache<String, Arc<ModelAssetBundle>>>,
    texture_cache: Mutex<BoundedCache<String, AssetPreviewSnapshot>>,
    last_request_cache_hit: AtomicBool,
    camera: AssetPreviewCameraState,
}
