use newengine_physics_api::{
    decode_json, encode_json, PhysicsBackendInfo, PhysicsFrameInput, PhysicsFrameOutput,
    PhysicsServiceRequest, PhysicsServiceResponse, PHYSICS_SERVICE_ID,
};
use newengine_plugin_api::HostApiV1;

use crate::service_runtime::GenericJsonServiceClient;

#[derive(Clone)]
pub(crate) struct PhysicsServiceClient {
    service: GenericJsonServiceClient,
}

impl PhysicsServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self { service: GenericJsonServiceClient::new(host, PHYSICS_SERVICE_ID) }
    }

    #[inline]
    pub(crate) fn info(&self) -> Result<PhysicsBackendInfo, String> {
        let bytes = self.service.info_json()?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(&self, req: PhysicsServiceRequest) -> Result<PhysicsServiceResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.invoke_json(payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn step(&self, input: PhysicsFrameInput) -> Result<PhysicsFrameOutput, String> {
        match self.invoke(PhysicsServiceRequest::StepFrame(input))? {
            PhysicsServiceResponse::FrameOutput(output) => Ok(output),
            PhysicsServiceResponse::Problem(problem) => Err(format!(
                "physics service problem {}: {} ({})",
                problem.code, problem.title, problem.detail
            )),
            other => Err(format!(
                "physics service protocol error: expected FrameOutput response, got {:?}",
                other
            )),
        }
    }
}
