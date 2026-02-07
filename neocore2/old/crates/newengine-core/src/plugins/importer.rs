#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetBlob, AssetError, AssetKey, BlobImporterDispatch, ImporterPriority};
use newengine_plugin_api::{Blob, CapabilityId, MethodName};
use std::sync::Arc;

use crate::plugins::describe::parse_describe;
use crate::plugins::host_api::call_service_v1;
use crate::plugins::host_context::ctx;

pub(crate) struct ServiceBlobImporter {
    stable_id: Arc<str>,
    exts: Vec<String>,
    output_type_id: Arc<str>,
    format: Arc<str>,
    method: Arc<str>,
    service_id: Arc<str>,
    priority: ImporterPriority,
}

impl ServiceBlobImporter {
    #[inline]
    fn call_import(&self, bytes: &[u8]) -> Result<Vec<u8>, AssetError> {
        let out: RResult<Blob, RString> = call_service_v1(
            CapabilityId::from(self.service_id.as_ref()),
            MethodName::from(self.method.as_ref()),
            Blob::from(bytes.to_vec()),
        );

        out.into_result()
            .map(|b| b.into_vec())
            .map_err(|e| AssetError::new(e.to_string()))
    }

    #[inline]
    fn unpack_wire_v1(frame: &[u8]) -> Result<(Arc<str>, Vec<u8>), AssetError> {
        if frame.len() < 4 {
            return Err(AssetError::new("importer wire v1: frame too small"));
        }

        let meta_len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let need = 4usize.saturating_add(meta_len);

        if frame.len() < need {
            return Err(AssetError::new("importer wire v1: truncated meta"));
        }

        let meta = &frame[4..4 + meta_len];
        let payload = frame[4 + meta_len..].to_vec();

        let meta_json = std::str::from_utf8(meta)
            .map_err(|_| AssetError::new("importer wire v1: meta is not utf8"))?
            .to_string();

        Ok((Arc::from(meta_json), payload))
    }
}

impl BlobImporterDispatch for ServiceBlobImporter {
    fn import_blob(&self, bytes: &[u8], _key: &AssetKey) -> Result<AssetBlob, AssetError> {
        let frame = self.call_import(bytes)?;
        let (meta_json, payload) = Self::unpack_wire_v1(&frame)?;

        Ok(AssetBlob {
            type_id: self.output_type_id.clone(),
            format: self.format.clone(),
            payload,
            meta_json,
            dependencies: Vec::new(),
        })
    }

    fn output_type_id(&self) -> Arc<str> {
        self.output_type_id.clone()
    }

    fn extensions(&self) -> Vec<String> {
        self.exts.clone()
    }

    fn priority(&self) -> ImporterPriority {
        self.priority
    }

    fn stable_id(&self) -> Arc<str> {
        self.stable_id.clone()
    }
}

pub(crate) fn try_auto_register_importer(service_id: &str, describe_json: &str) {
    let Some(d) = parse_describe(describe_json) else {
        return;
    };

    if d.kind.as_deref() != Some("asset_importer") {
        return;
    }
    let Some(imp) = d.asset_importer else {
        return;
    };

    let _wire = imp.wire;

    let importer = ServiceBlobImporter {
        stable_id: Arc::from(service_id.to_string()),
        exts: imp.extensions,
        output_type_id: Arc::from(imp.output_type_id),
        format: Arc::from(imp.format),
        method: Arc::from(imp.method),
        service_id: Arc::from(service_id.to_string()),
        priority: ImporterPriority::new(imp.priority.unwrap_or(0)),
    };

    ctx().asset_store.add_importer(Arc::new(importer));
    log::info!(target: "assets", "importer.auto_registered service_id='{}'", service_id);
}
