use super::AssetServiceClient;
use crate::{
    AssetService, NepakPackageWriteRequestV1, NepakPackageWriteResponseV1, TextAssetWriteRequestV1,
    TextAssetWriteResponseV1,
};

impl AssetService for AssetServiceClient {
    fn reload(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call_raw(self.m_reload.clone(), Self::logical_payload(logical_path))?;
        Self::decode_load_like(bytes, "reload_v1")
    }

    fn info_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.call_logical_json(self.m_info_json_v1.clone(), logical_path)
    }

    fn formats_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_empty_json(self.m_formats_json_v1.clone())
    }

    fn sources_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_empty_json(self.m_sources_json_v1.clone())
    }

    fn vfs_list_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.call_logical_json(self.m_vfs_list_json_v1.clone(), logical_path)
    }

    fn list_file_repack_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        AssetServiceClient::list_file_repack_json_v1(self, payload)
    }

    fn uid_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.call_logical_json(self.m_uid_json_v1.clone(), logical_path)
    }

    fn import_cache_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_cache_json_v1.clone(), &payload)
    }

    fn import_dirty_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_dirty_json_v1.clone(), &payload)
    }

    fn import_scan_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_scan_json_v1.clone(), &payload)
    }

    fn import_graph_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_graph_json_v1.clone(), &payload)
    }

    fn runtime_graph_json_v1(&self) -> Result<crate::AssetRuntimeGraphV1, String> {
        self.call_json_typed(
            self.m_runtime_graph_json_v1.clone(),
            &serde_json::Value::Null,
            "runtime_graph_json_v1",
        )
    }

    fn import_diagnostics_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_diagnostics_json_v1.clone(), &payload)
    }

    fn import_thumbnails_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_thumbnails_json_v1.clone(), &payload)
    }

    fn import_dependencies_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_dependencies_json_v1.clone(), &payload)
    }

    fn import_queue_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_import_queue_json_v1.clone(), &payload)
    }

    fn reimport_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_reimport_v1.clone(), &payload)
    }

    fn thumbnail_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_thumbnail_json_v1.clone(), &payload)
    }

    fn dirty_scan_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_dirty_scan_json_v1.clone(), &payload)
    }

    fn package_writer_info_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json_value(self.m_package_writer_info_json_v1.clone(), &payload)
    }

    fn package_write_nepak_json_v1(
        &self,
        payload: NepakPackageWriteRequestV1,
    ) -> Result<NepakPackageWriteResponseV1, String> {
        self.call_json_typed(
            self.m_package_write_nepak_json_v1.clone(),
            &payload,
            "package_write_nepak_json_v1",
        )
    }

    fn package_write_text_json_v1(
        &self,
        payload: TextAssetWriteRequestV1,
    ) -> Result<TextAssetWriteResponseV1, String> {
        self.call_json_typed(
            self.m_package_write_text_json_v1.clone(),
            &payload,
            "package_write_text_json_v1",
        )
    }

    fn mount_source_json_v1(&self, payload: serde_json::Value) -> Result<(), String> {
        self.call_json_unit(self.m_mount_source_json_v1.clone(), &payload)
    }

    fn resolve_trace_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.call_logical_json(self.m_resolve_trace_json_v1.clone(), logical_path)
    }
}
