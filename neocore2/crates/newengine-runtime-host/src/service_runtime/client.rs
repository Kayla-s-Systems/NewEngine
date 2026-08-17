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
    info_method: MethodName,
    invoke_method: MethodName,
}

impl GenericJsonServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1, service_id: &str) -> Self {
        Self {
            host,
            service_id: RString::from(service_id),
            info_method: MethodName::from(SERVICE_METHOD_INFO_JSON),
            invoke_method: MethodName::from(SERVICE_METHOD_INVOKE_JSON),
        }
    }

    #[inline]
    pub(crate) fn call_raw(&self, method_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let method = MethodName::from(method_name);
        self.call_raw_method(&method, payload)
    }

    #[inline]
    pub(crate) fn call_raw_method(
        &self,
        method: &MethodName,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), method.clone(), Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())
    }

    #[inline]
    pub(crate) fn info_json(&self) -> Result<Vec<u8>, String> {
        self.call_raw_method(&self.info_method, Vec::new())
    }

    #[inline]
    pub(crate) fn invoke_json(&self, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        self.call_raw_method(&self.invoke_method, payload)
    }
}
