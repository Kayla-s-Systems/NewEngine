use newengine_core::physics::{
    PhysicsApi, PhysicsBackendInfo, PhysicsFrameInput, PhysicsFrameOutput,
};
use newengine_core::{EngineError, EngineResult};

use crate::client::PhysicsServiceClient;

pub(crate) struct ServiceBackedPhysicsApi {
    client: PhysicsServiceClient,
}

impl ServiceBackedPhysicsApi {
    #[inline]
    pub(crate) fn new(client: PhysicsServiceClient) -> Self {
        Self { client }
    }
}

impl PhysicsApi for ServiceBackedPhysicsApi {
    #[inline]
    fn backend_info(&mut self) -> EngineResult<PhysicsBackendInfo> {
        self.client.info().map_err(EngineError::other)
    }

    #[inline]
    fn step_frame(&mut self, input: PhysicsFrameInput) -> EngineResult<PhysicsFrameOutput> {
        self.client.step(input).map_err(EngineError::other)
    }
}
