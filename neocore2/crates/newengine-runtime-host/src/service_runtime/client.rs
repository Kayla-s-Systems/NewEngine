use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_api::{SERVICE_METHOD_INFO_JSON, SERVICE_METHOD_INVOKE_JSON};

/// Generic transport client for plugin services that expose the standard
/// JSON-control surface: `info_json` and `invoke_json`.
///
/// Domain adapters remain responsible for DTO encoding/decoding and typed
/// protocol validation. This client only owns the ABI call plumbing.
#[derive(Clone)]
pub(crate) struct GenericJsonServiceClient {
    host: HostApiV1,
    service_id: RString,
}

impl GenericJsonServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1, service_id: &'static str) -> Self {
        Self { host, service_id: RString::from(service_id) }
    }

    #[inline]
    pub(crate) fn call_raw(&self, method_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|err| err.to_string())
    }

    #[inline]
    pub(crate) fn info_json(&self) -> Result<Vec<u8>, String> {
        self.call_raw(SERVICE_METHOD_INFO_JSON, Vec::new())
    }

    #[inline]
    pub(crate) fn invoke_json(&self, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        self.call_raw(SERVICE_METHOD_INVOKE_JSON, payload)
    }
}
