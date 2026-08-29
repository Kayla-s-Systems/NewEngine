#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::Arc;

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::json_service::{decode_json_payload, ok_empty_blob, ok_json, payload_json};

type RouteHandler<S> =
    Box<dyn Fn(&Arc<Mutex<S>>, Blob) -> RResult<Blob, RString> + Send + Sync + 'static>;

pub struct JsonServiceRouter<S = ()> {
    service_id: String,
    description_json: String,
    state: Arc<Mutex<S>>,
    routes: BTreeMap<String, RouteHandler<S>>,
}

impl JsonServiceRouter<()> {
    #[inline]
    pub fn new(service_id: impl Into<String>) -> Self {
        Self::with_state(service_id, ())
    }
}

impl<S: Send + 'static> JsonServiceRouter<S> {
    #[inline]
    pub fn with_state(service_id: impl Into<String>, state: S) -> Self {
        Self::with_shared_state(service_id, Arc::new(Mutex::new(state)))
    }

    #[inline]
    pub fn with_shared_state(service_id: impl Into<String>, state: Arc<Mutex<S>>) -> Self {
        let service_id = service_id.into();
        let description_json = serde_json::json!({
            "id": service_id.clone(),
            "version": 1,
            "methods": []
        })
        .to_string();
        Self {
            service_id,
            description_json,
            state,
            routes: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn describe_json<T: Serialize>(mut self, description: &T) -> Self {
        self.description_json =
            serde_json::to_string(description).unwrap_or_else(|_| "{}".to_owned());
        self
    }

    #[inline]
    pub fn info<T, F>(self, handler: F) -> Self
    where
        T: Serialize + 'static,
        F: Fn() -> T + Send + Sync + 'static,
    {
        self.get_json(newengine_service_api::SERVICE_METHOD_INFO_JSON, move |_| {
            handler()
        })
    }

    #[inline]
    pub fn info_result<T, F>(self, handler: F) -> Self
    where
        T: Serialize + 'static,
        F: Fn() -> Result<T, String> + Send + Sync + 'static,
    {
        self.get_json_result(newengine_service_api::SERVICE_METHOD_INFO_JSON, move |_| {
            handler()
        })
    }

    #[inline]
    pub fn get_json<T, F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        T: Serialize + 'static,
        F: Fn(&mut S) -> T + Send + Sync + 'static,
    {
        let method = method.into();
        self.routes.insert(
            method,
            Box::new(move |state, _payload| {
                let mut state = state.lock();
                ok_json(handler(&mut state))
            }),
        );
        self
    }

    #[inline]
    pub fn get_json_result<T, F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        T: Serialize + 'static,
        F: Fn(&mut S) -> Result<T, String> + Send + Sync + 'static,
    {
        let method = method.into();
        self.routes.insert(
            method,
            Box::new(move |state, _payload| {
                let mut state = state.lock();
                match handler(&mut state) {
                    Ok(value) => ok_json(&value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }),
        );
        self
    }

    #[inline]
    pub fn post_json<I, O, F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        I: DeserializeOwned + 'static,
        O: Serialize + 'static,
        F: Fn(&mut S, I) -> O + Send + Sync + 'static,
    {
        let method = method.into();
        let service_id = self.service_id.clone();
        self.routes.insert(
            method.clone(),
            Box::new(move |state, payload| {
                let request = match decode_json_payload::<I>(&service_id, &method, &payload) {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(e),
                };
                let mut state = state.lock();
                ok_json(handler(&mut state, request))
            }),
        );
        self
    }

    #[inline]
    pub fn post_json_result<I, O, F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        I: DeserializeOwned + 'static,
        O: Serialize + 'static,
        F: Fn(&mut S, I) -> Result<O, String> + Send + Sync + 'static,
    {
        let method = method.into();
        let service_id = self.service_id.clone();
        self.routes.insert(
            method.clone(),
            Box::new(move |state, payload| {
                let request = match decode_json_payload::<I>(&service_id, &method, &payload) {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(e),
                };
                let mut state = state.lock();
                match handler(&mut state, request) {
                    Ok(value) => ok_json(&value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }),
        );
        self
    }

    #[inline]
    pub fn put_json<I, O, F>(self, method: impl Into<String>, handler: F) -> Self
    where
        I: DeserializeOwned + 'static,
        O: Serialize + 'static,
        F: Fn(&mut S, I) -> O + Send + Sync + 'static,
    {
        self.post_json(method, handler)
    }

    #[inline]
    pub fn put_json_result<I, O, F>(self, method: impl Into<String>, handler: F) -> Self
    where
        I: DeserializeOwned + 'static,
        O: Serialize + 'static,
        F: Fn(&mut S, I) -> Result<O, String> + Send + Sync + 'static,
    {
        self.post_json_result(method, handler)
    }

    #[inline]
    pub fn json_value_result<F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&mut S, serde_json::Value) -> Result<serde_json::Value, String>
            + Send
            + Sync
            + 'static,
    {
        let method = method.into();
        self.routes.insert(
            method,
            Box::new(move |state, payload| {
                let request = match payload_json(&payload) {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                let mut state = state.lock();
                match handler(&mut state, request) {
                    Ok(value) => ok_json(&value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }),
        );
        self
    }

    #[inline]
    pub fn blob<F>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&mut S, Blob) -> RResult<Blob, RString> + Send + Sync + 'static,
    {
        let method = method.into();
        self.routes.insert(
            method,
            Box::new(move |state, payload| {
                let mut state = state.lock();
                handler(&mut state, payload)
            }),
        );
        self
    }

    #[inline]
    pub fn shutdown(self) -> Self {
        self.blob(
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
            |_state, _payload| ok_empty_blob(),
        )
    }

    #[inline]
    pub fn shutdown_json<T, F>(self, handler: F) -> Self
    where
        T: Serialize + 'static,
        F: Fn(&mut S) -> T + Send + Sync + 'static,
    {
        self.get_json(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1, handler)
    }

    #[inline]
    pub fn into_service_v1(self) -> ServiceV1Dyn<'static> {
        ServiceV1Dyn::from_value(self, TD_Opaque)
    }
}

impl<S: Send + 'static> ServiceV1 for JsonServiceRouter<S> {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(self.service_id.as_str())
    }

    fn describe(&self) -> RString {
        RString::from(self.description_json.clone())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let method_name = method.as_str();
        match self.routes.get(method_name) {
            Some(handler) => handler(&self.state, payload),
            None => RResult::RErr(RString::from(format!(
                "{}: unknown method '{}'",
                self.service_id, method_name
            ))),
        }
    }
}
