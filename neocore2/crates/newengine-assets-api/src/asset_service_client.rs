#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{HostApiV1, MethodName};

use crate::{method, ASSET_SERVICE_ID};

mod access;
mod core;
mod editor;
mod service;
mod streaming;
mod textures;
mod transport;
mod types;

/// Thin client over the engine AssetManager service.
///
/// The AssetManager is an engine service (typically provided by a runtime plugin).
/// This client performs service calls through `HostApiV1` and does not link
/// against any concrete AssetManager implementation.
#[derive(Clone)]
pub struct AssetServiceClient {
    host: HostApiV1,
    service_id: RString,

    // Cache MethodName allocations; clones are cheap.
    m_import_v1: MethodName,
    m_reload: MethodName,
    m_pump: MethodName,
    m_info_json_v1: MethodName,
    m_blob_wire_v1: MethodName,
    m_text_v1: MethodName,
    m_raw_bytes_v1: MethodName,
    m_raw_range_v1: MethodName,
    m_texture_rgba8_v1: MethodName,
    m_decode_v1: MethodName,
    m_texture_dictionary_rgba8_v1: MethodName,
    m_texture_dictionary_runtime_v1: MethodName,
    m_status_json_v1: MethodName,
    m_status_graph_json_v1: MethodName,
    m_project_status_json_v1: MethodName,
    m_resolve_trace_json_v1: MethodName,
    m_formats_json_v1: MethodName,
    m_sources_json_v1: MethodName,
    m_vfs_list_json_v1: MethodName,
    m_list_file_repack_json_v1: MethodName,
    m_uid_json_v1: MethodName,
    m_import_cache_json_v1: MethodName,
    m_import_dirty_json_v1: MethodName,
    m_import_scan_json_v1: MethodName,
    m_import_graph_json_v1: MethodName,
    m_runtime_graph_json_v1: MethodName,
    m_import_diagnostics_json_v1: MethodName,
    m_import_thumbnails_json_v1: MethodName,
    m_import_dependencies_json_v1: MethodName,
    m_import_queue_json_v1: MethodName,
    m_reimport_v1: MethodName,
    m_thumbnail_json_v1: MethodName,
    m_dirty_scan_json_v1: MethodName,
    m_package_writer_info_json_v1: MethodName,
    m_package_write_nepak_json_v1: MethodName,
    m_package_write_text_json_v1: MethodName,
    m_mount_source_json_v1: MethodName,
    m_get_state_v1: MethodName,
}

impl AssetServiceClient {
    /// Create a client bound to the canonical AssetManager service API.
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self::for_service(host, ASSET_SERVICE_ID)
    }

    /// Create a client bound to an explicit service id.
    ///
    /// This is intended for host/bootstrap orchestration before the stable
    /// `engine.assets` gateway has been published. Runtime consumers should
    /// continue to use [`Self::new`].
    #[inline]
    pub fn for_service(host: HostApiV1, service_id: impl Into<RString>) -> Self {
        Self {
            host,
            service_id: service_id.into(),

            m_import_v1: MethodName::from(method::IMPORT_V1),
            m_reload: MethodName::from(method::RELOAD_V1),
            m_pump: MethodName::from(method::PUMP_V1),
            m_info_json_v1: MethodName::from(method::INFO_JSON_V1),
            m_blob_wire_v1: MethodName::from(method::BLOB_WIRE_V1),
            m_text_v1: MethodName::from(method::TEXT_V1),
            m_raw_bytes_v1: MethodName::from(method::RAW_BYTES_V1),
            m_raw_range_v1: MethodName::from(method::RAW_RANGE_V1),
            m_texture_rgba8_v1: MethodName::from(method::TEXTURE_RGBA8_V1),
            m_decode_v1: MethodName::from(method::DECODE_V1),
            m_texture_dictionary_rgba8_v1: MethodName::from(method::TEXTURE_DICTIONARY_RGBA8_V1),
            m_texture_dictionary_runtime_v1: MethodName::from(
                method::TEXTURE_DICTIONARY_RUNTIME_V1,
            ),
            m_status_json_v1: MethodName::from(method::STATUS_JSON_V1),
            m_status_graph_json_v1: MethodName::from(method::STATUS_GRAPH_JSON_V1),
            m_project_status_json_v1: MethodName::from(method::PROJECT_STATUS_JSON_V1),
            m_resolve_trace_json_v1: MethodName::from(method::RESOLVE_TRACE_JSON_V1),
            m_formats_json_v1: MethodName::from(method::FORMATS_JSON_V1),
            m_sources_json_v1: MethodName::from(method::SOURCES_JSON_V1),
            m_vfs_list_json_v1: MethodName::from(method::VFS_LIST_JSON_V1),
            m_list_file_repack_json_v1: MethodName::from(method::LIST_FILE_REPACK_JSON_V1),
            m_uid_json_v1: MethodName::from(method::UID_JSON_V1),
            m_import_cache_json_v1: MethodName::from(method::IMPORT_CACHE_JSON_V1),
            m_import_dirty_json_v1: MethodName::from(method::IMPORT_DIRTY_JSON_V1),
            m_import_scan_json_v1: MethodName::from(method::IMPORT_SCAN_JSON_V1),
            m_import_graph_json_v1: MethodName::from(method::IMPORT_GRAPH_JSON_V1),
            m_runtime_graph_json_v1: MethodName::from(method::RUNTIME_GRAPH_JSON_V1),
            m_import_diagnostics_json_v1: MethodName::from(method::IMPORT_DIAGNOSTICS_JSON_V1),
            m_import_thumbnails_json_v1: MethodName::from(method::IMPORT_THUMBNAILS_JSON_V1),
            m_import_dependencies_json_v1: MethodName::from(method::IMPORT_DEPENDENCIES_JSON_V1),
            m_import_queue_json_v1: MethodName::from(method::IMPORT_QUEUE_JSON_V1),
            m_reimport_v1: MethodName::from(method::REIMPORT_V1),
            m_thumbnail_json_v1: MethodName::from(method::THUMBNAIL_JSON_V1),
            m_dirty_scan_json_v1: MethodName::from(method::DIRTY_SCAN_JSON_V1),
            m_package_writer_info_json_v1: MethodName::from(method::PACKAGE_WRITER_INFO_JSON_V1),
            m_package_write_nepak_json_v1: MethodName::from(method::PACKAGE_WRITE_NEPAK_JSON_V1),
            m_package_write_text_json_v1: MethodName::from(method::PACKAGE_WRITE_TEXT_JSON_V1),
            m_mount_source_json_v1: MethodName::from(method::MOUNT_SOURCE_JSON_V1),
            m_get_state_v1: MethodName::from(method::GET_STATE_V1),
        }
    }

    #[inline]
    pub fn service_id(&self) -> &RString {
        &self.service_id
    }
}
