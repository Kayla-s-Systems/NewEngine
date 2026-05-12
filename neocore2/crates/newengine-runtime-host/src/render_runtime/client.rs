use std::sync::atomic::{AtomicBool, Ordering};

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_render_api::{
    decode_json, encode_json, render_legacy_protocol_warning, RenderApiVersion, RenderBackendInfoV1,
    RenderBackendInfoV3, RenderRequestV1, RenderRequestV3, RenderResponseV1, RenderResponseV3,
    RENDER_SERVICE_ID, RENDER_SERVICE_METHOD_INFO_V1, RENDER_SERVICE_METHOD_INFO_V3,
    RENDER_SERVICE_METHOD_INVOKE_V1, RENDER_SERVICE_METHOD_INVOKE_V3,
};

static WARNED_V1_FALLBACK: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub(crate) struct RenderServiceClient {
    host: HostApiV1,
    service_id: RString,
}

impl RenderServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(RENDER_SERVICE_ID),
        }
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), method_name, Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())
    }

    #[inline]
    fn warn_legacy_fallback_once(version: RenderApiVersion, reason: &str) {
        if !WARNED_V1_FALLBACK.swap(true, Ordering::Relaxed) {
            let warning = render_legacy_protocol_warning(version);
            log::warn!(
                "{}: {} fallback_reason='{}' migration_target=v{}.{}.{}",
                warning.code,
                warning.message,
                reason,
                warning.migration_target.major,
                warning.migration_target.minor,
                warning.migration_target.patch,
            );
        }
    }

    #[inline]
    pub(crate) fn info_v3(&self) -> Result<RenderBackendInfoV3, String> {
        match self.call(MethodName::from(RENDER_SERVICE_METHOD_INFO_V3), Vec::new()) {
            Ok(bytes) => decode_json(&bytes),
            Err(v3_err) => {
                Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                let bytes = self.call(MethodName::from(RENDER_SERVICE_METHOD_INFO_V1), Vec::new())?;
                let info: RenderBackendInfoV1 = decode_json(&bytes)?;
                Ok(RenderBackendInfoV3::from_v1(info))
            }
        }
    }

    #[inline]
    pub(crate) fn invoke_v3(&self, req: RenderRequestV3) -> Result<RenderResponseV3, String> {
        let payload = encode_json(&req)?;
        match self.call(MethodName::from(RENDER_SERVICE_METHOD_INVOKE_V3), payload) {
            Ok(bytes) => decode_json(&bytes),
            Err(v3_err) => match req {
                RenderRequestV3::Immediate(req_v1) => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    self.invoke_v1(req_v1).map(RenderResponseV3::Immediate)
                }
                RenderRequestV3::V1(req_v1) => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    self.invoke_v1(req_v1).map(RenderResponseV3::V1)
                }
                RenderRequestV3::SetWorkBudget(budget) => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    self.invoke_v1(RenderRequestV1::SetWorkBudget(budget))?;
                    Ok(RenderResponseV3::Unit)
                }
                RenderRequestV3::PumpUploads(desc) => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    match self.invoke_v1(RenderRequestV1::PumpUploads(desc))? {
                        RenderResponseV1::UploadPumpReport(report) => {
                            Ok(RenderResponseV3::UploadPumpReport(report))
                        }
                        other => Err(format!(
                            "render service protocol error: expected UploadPumpReport from V1 fallback, got {:?}",
                            other
                        )),
                    }
                }
                RenderRequestV3::DiagnosticsSnapshot => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    match self.invoke_v1(RenderRequestV1::DiagnosticsSnapshot)? {
                        RenderResponseV1::DiagnosticsSnapshot(snapshot) => {
                            Ok(RenderResponseV3::DiagnosticsSnapshot(snapshot))
                        }
                        other => Err(format!(
                            "render service protocol error: expected DiagnosticsSnapshot from V1 fallback, got {:?}",
                            other
                        )),
                    }
                }
                RenderRequestV3::SetRenderPhase { .. }
                | RenderRequestV3::SetDrawListKind { .. }
                | RenderRequestV3::DiscardRecordedCommands => {
                    Self::warn_legacy_fallback_once(RenderApiVersion::new(1, 0, 0), &v3_err);
                    Ok(RenderResponseV3::Unit)
                }
                _ => Err(v3_err),
            },
        }
    }

    #[inline]
    pub(crate) fn invoke(&self, req: RenderRequestV1) -> Result<RenderResponseV1, String> {
        match self.invoke_v3(RenderRequestV3::Immediate(req))? {
            RenderResponseV3::Immediate(response) => Ok(response),
            RenderResponseV3::V1(response) => Ok(response),
            RenderResponseV3::Problem(problem) => Err(format!(
                "render service problem {}: {} ({})",
                problem.code, problem.title, problem.detail
            )),
            other => Err(format!(
                "render service protocol error: expected V1 response wrapper, got {:?}",
                other
            )),
        }
    }

    #[inline]
    fn invoke_v1(&self, req: RenderRequestV1) -> Result<RenderResponseV1, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(MethodName::from(RENDER_SERVICE_METHOD_INVOKE_V1), payload)?;
        decode_json(&bytes)
    }
}
