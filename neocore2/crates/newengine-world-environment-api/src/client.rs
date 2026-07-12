use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    EnvironmentFrameDto, EnvironmentFrameRequest, EnvironmentSnapshotRequest,
    EnvironmentSnapshotResponse, ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};

/// Thin host-side client over `engine.world.environment`.
#[derive(Clone)]
pub struct EnvironmentClient {
    host: HostApiV1,
    service_id: RString,
}

impl EnvironmentClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_WORLD_ENVIRONMENT_SERVICE_ID),
        }
    }

    #[inline]
    fn call_json<Request, Response>(
        &self,
        method: &str,
        request: &Request,
    ) -> Result<Response, String>
    where
        Request: Serialize + ?Sized,
        Response: DeserializeOwned,
    {
        let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
        let response = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method),
            Blob::from(payload),
        );
        let bytes = response
            .into_result()
            .map_err(|error| error.to_string())?
            .into_vec();
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    #[inline]
    pub fn frame_json_v1(
        &self,
        request: EnvironmentFrameRequest,
    ) -> Result<EnvironmentFrameDto, String> {
        self.call_json(WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, &request)
    }

    #[inline]
    pub fn snapshot_json_v1(&self) -> Result<EnvironmentSnapshotResponse, String> {
        self.call_json(
            WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            &EnvironmentSnapshotRequest::default(),
        )
    }
}
