use serde::{de::DeserializeOwned, Serialize};

#[inline]
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| error.to_string())
}

#[inline]
pub fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}
