use super::AssetServiceClient;
use crate::AssetDecodeRequest;

impl AssetServiceClient {
    /// Enqueue importer-owned asset import by logical path.
    #[inline]
    pub fn import_v1(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call_raw(
            self.m_import_v1.clone(),
            Self::logical_payload(logical_path),
        )?;
        Self::decode_load_like(bytes, "import_v1")
    }

    /// Read raw bytes from the AssetManager VFS by logical path.
    ///
    /// This intentionally bypasses importers, but it does not bypass AssetManager: resolution
    /// still goes through the mounted VFS layers (.nepak, filesystem, future remote sources).
    #[inline]
    pub fn raw_bytes_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        self.call_raw(
            self.m_raw_bytes_v1.clone(),
            Self::logical_payload(logical_path),
        )
    }

    /// Read UTF-8/text asset bytes directly through the AssetManager v1 text method.
    #[inline]
    pub fn text_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        self.call_raw(self.m_text_v1.clone(), Self::logical_payload(logical_path))
    }

    /// Validated lifecycle projection for systems that own non-CPU residency,
    /// for example the render controller marking GPU upload/residency stages.
    #[inline]
    pub fn project_status_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        Self::decode_ok_json(self.call_raw(self.m_project_status_json_v1.clone(), bytes)?)
    }

    #[inline]
    pub fn decode_v1(&self, request: &AssetDecodeRequest) -> Result<Vec<u8>, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        self.call_raw(self.m_decode_v1.clone(), payload)
    }

    /// List a mounted VFS directory through AssetManager.
    #[inline]
    pub fn vfs_list_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let bytes = self.call_raw(
            self.m_vfs_list_json_v1.clone(),
            Self::logical_payload(logical_path),
        )?;
        Self::decode_ok_json(bytes)
    }

    /// Repack a NEF8 ListFile after editor-side entry mutation and write it back through AssetManager VFS.
    #[inline]
    pub fn list_file_repack_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(self.m_list_file_repack_json_v1.clone(), bytes)?;
        Self::decode_ok_json(bytes)
    }
}
