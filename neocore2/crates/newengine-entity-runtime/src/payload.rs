use abi_stable::std_types::RString;
use newengine_plugin_api::Blob;
use serde::de::DeserializeOwned;

pub(crate) fn decode_payload<T>(payload: &Blob) -> Result<T, RString>
where
    T: DeserializeOwned,
{
    newengine_service_kit::payload_json(payload)
        .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        .map_err(RString::from)
}

pub(crate) fn payload_from_value(value: &serde_json::Value) -> Result<Blob, RString> {
    serde_json::to_vec(value)
        .map(Blob::from)
        .map_err(|error| RString::from(error.to_string()))
}
