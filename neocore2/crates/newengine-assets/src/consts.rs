#![forbid(unsafe_op_in_unsafe_fn)]

/// Default service id for the AssetManager service.
///
/// This is a runtime service provided by a plugin (typically `newengine-AssetManager`).
pub const ASSET_SERVICE_ID: &str = "asset.manager";

/// Canonical method names for the AssetManager service.
///
/// Method naming is **contract-first** and stable across versions.
pub mod method {
    pub const LOAD: &str = "asset.load";
    pub const RELOAD: &str = "asset.reload";
    pub const PUMP: &str = "asset.pump";
    pub const INFO_JSON: &str = "asset.info_json";
    pub const STATE_JSON: &str = "asset.state_json";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";

    pub const FORMATS_JSON: &str = "asset.formats_json";
    pub const SOURCES_JSON: &str = "asset.sources_json";
    pub const MOUNT_PAK: &str = "asset.mount_pak";
    pub const MOUNT_DIR: &str = "asset.mount_dir";
}
