use std::fmt::Display;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use serde::de::DeserializeOwned;

pub(crate) fn decode_json<T>(payload: &Blob, context: &str) -> Result<T, RString>
where
    T: DeserializeOwned,
{
    serde_json::from_slice(payload.as_slice())
        .map_err(|error| RString::from(format!("{context}: {error}")))
}

pub(crate) fn handle_binary<T, R, E>(
    bytes: &[u8],
    decode: impl FnOnce(&[u8]) -> Result<T, E>,
    handle: impl FnOnce(T) -> R,
    encode: impl FnOnce(&R) -> Vec<u8>,
    context: &str,
) -> RResult<Blob, RString>
where
    E: Display,
{
    match decode(bytes) {
        Ok(request) => {
            let response = handle(request);
            RResult::ROk(Blob::from(encode(&response)))
        }
        Err(error) => RResult::RErr(RString::from(format!("{context}: {error}"))),
    }
}
