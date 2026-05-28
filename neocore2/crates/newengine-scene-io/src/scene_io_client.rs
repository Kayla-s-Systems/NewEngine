#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

use crate::consts::{method, ENGINE_SCENE_SERVICE_ID};

/// Thin client over the engine scene gateway.
///
/// The scene gateway can be backed by a plugin provider or by an engine-runtime
/// service source. This client performs calls through `HostApiV1` and does not
/// link against a concrete implementation.
#[derive(Clone)]
pub struct SceneIoClient {
    host: HostApiV1,
    service_id: RString,

    m_formats_json: MethodName,
    m_load_json_v1: MethodName,
    m_save_json_v1: MethodName,
    m_graph_json_v1: MethodName,
    m_archetype_graph_json_v1: MethodName,
    m_placements_json_v1: MethodName,
}

impl SceneIoClient {
    /// Create a client bound to the given host API.
    ///
    /// Service id defaults to [`ENGINE_SCENE_SERVICE_ID`].
    ///
    /// Scene consumers always call the stable `engine.scene` facade id.
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_SCENE_SERVICE_ID),

            m_formats_json: MethodName::from(method::FORMATS_JSON),
            m_load_json_v1: MethodName::from(method::LOAD_JSON_V1),
            m_save_json_v1: MethodName::from(method::SAVE_JSON_V1),
            m_graph_json_v1: MethodName::from(method::GRAPH_JSON_V1),
            m_archetype_graph_json_v1: MethodName::from(method::ARCHETYPE_GRAPH_JSON_V1),
            m_placements_json_v1: MethodName::from(method::PLACEMENTS_JSON_V1),
        }
    }

    #[inline]
    pub fn service_id(&self) -> &RString {
        &self.service_id
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            method_name,
            Blob::from(payload),
        );

        res.into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())
    }

    #[inline]
    fn decode_utf8(bytes: Vec<u8>) -> Result<String, String> {
        String::from_utf8(bytes).map_err(|_| "engine.scene service returned non-utf8".to_string())
    }

    #[inline]
    fn parse_json(s: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())
    }

    #[inline]
    fn decode_ok_json(bytes: Vec<u8>) -> Result<serde_json::Value, String> {
        let s = Self::decode_utf8(bytes)?;
        let v = Self::parse_json(&s)?;
        Ok(v)
    }

    #[inline]
    pub fn formats_json(&self) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call(self.m_formats_json.clone(), Vec::new())?)
    }

    #[inline]
    pub fn graph_json_v1(&self) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call(self.m_graph_json_v1.clone(), Vec::new())?)
    }

    #[inline]
    pub fn archetype_graph_json_v1(&self) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call(self.m_archetype_graph_json_v1.clone(), Vec::new())?)
    }

    #[inline]
    pub fn placements_json_v1(&self) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call(self.m_placements_json_v1.clone(), Vec::new())?)
    }

    #[inline]
    pub fn load_json_v1(&self, path: &str, replace: bool) -> Result<serde_json::Value, String> {
        let req = serde_json::json!({
            "path": path,
            "replace": replace,
        });
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        Self::decode_ok_json(self.call(self.m_load_json_v1.clone(), payload)?)
    }

    #[inline]
    pub fn save_json_v1(
        &self,
        path: &str,
        pretty: bool,
        include_empty_entities: bool,
    ) -> Result<serde_json::Value, String> {
        let req = serde_json::json!({
            "path": path,
            "pretty": pretty,
            "options": {
                "include_empty_entities": include_empty_entities,
            }
        });
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        Self::decode_ok_json(self.call(self.m_save_json_v1.clone(), payload)?)
    }
}
