#![forbid(unsafe_op_in_unsafe_fn)]

/// Default service id for Scene IO.
///
/// This is a runtime service, typically provided by a plugin. The editor may
/// register a host fallback implementation when no plugin is present.
pub const SCENE_IO_SERVICE_ID: &str = "scene.io";

/// Canonical method names for Scene IO.
///
/// Method naming is contract-first and stable across versions.
pub mod method {
    /// Returns a JSON descriptor of supported scene formats.
    pub const FORMATS_JSON: &str = "scene.formats_json";

    /// Load a scene from a JSON payload stored at `path`.
    ///
    /// Request payload: json `{ path, replace, options }`.
    pub const LOAD_JSON_V1: &str = "scene.load_json_v1";

    /// Save the current scene into a JSON payload stored at `path`.
    ///
    /// Request payload: json `{ path, pretty, options }`.
    pub const SAVE_JSON_V1: &str = "scene.save_json_v1";
}
