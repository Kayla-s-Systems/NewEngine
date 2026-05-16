use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_physics_api::{
    decode_json, encode_json, PhysicsBackendInfo, PhysicsFrameInput, PhysicsFrameOutput,
    PhysicsServiceRequest, PhysicsServiceResponse, PHYSICS_SERVICE_ID,
    PHYSICS_SERVICE_METHOD_INFO, PHYSICS_SERVICE_METHOD_INVOKE,
};

#[derive(Clone)]
pub(crate) struct PhysicsServiceClient {
    host: HostApiV1,
    service_id: RString,
}

impl PhysicsServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self { host, service_id: RString::from(PHYSICS_SERVICE_ID) }
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), method_name, Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())
    }

    #[inline]
    pub(crate) fn info(&self) -> Result<PhysicsBackendInfo, String> {
        let bytes = self.call(MethodName::from(PHYSICS_SERVICE_METHOD_INFO), Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(&self, req: PhysicsServiceRequest) -> Result<PhysicsServiceResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(MethodName::from(PHYSICS_SERVICE_METHOD_INVOKE), payload)?;
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
