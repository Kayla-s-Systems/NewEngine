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
    /// Raw VFS bytes by logical path. This bypasses importers but still resolves exclusively through AssetManager mounts.
    pub const RAW_BYTES_V1: &str = "asset.raw_bytes_v1";

    // Fast-path / batch APIs.
    pub const PRELOAD_MANY_V1: &str = "asset.preload_many_v1";
    pub const GET_STATE_V1: &str = "asset.get_state_v1";

    pub const FORMATS_JSON: &str = "asset.formats_json";
    pub const SOURCES_JSON: &str = "asset.sources_json";
    pub const MOUNT_PAK: &str = "asset.mount_pak";
    pub const MOUNT_DIR: &str = "asset.mount_dir";

    // VFS layered mounting (priority-driven).
    pub const MOUNT_PAK_PRIO: &str = "asset.mount_pak_prio";
    pub const MOUNT_DIR_PRIO: &str = "asset.mount_dir_prio";

    // Debug/diagnostics.
    pub const RESOLVE_TRACE_JSON: &str = "asset.resolve_trace_json";
}
