use newengine_audio_api::{
    AudioMusicSessionId, AudioObjectId, InteractiveMusicGraph, InteractiveMusicRuntimeState,
};

use crate::audio_orchestration::AudioOrchestrationHandle;

/// Project-facing InteractiveMusicGraph capability. The implementation shares the bounded
/// orchestration command queue, but exposes only graph/session/state/RTPC operations.
#[derive(Clone)]
pub struct InteractiveMusicHandle {
    orchestration: AudioOrchestrationHandle,
}

impl InteractiveMusicHandle {
    pub(crate) fn new(orchestration: AudioOrchestrationHandle) -> Self {
        Self { orchestration }
    }

    pub fn install_graph(&self, graph: InteractiveMusicGraph) -> Result<(), String> {
        self.orchestration.install_music_graph(graph)
    }

    pub fn create_session(
        &self,
        graph: impl Into<String>,
        object_id: AudioObjectId,
    ) -> Result<AudioMusicSessionId, String> {
        self.orchestration
            .create_music_session(graph.into(), object_id)
    }

    pub fn destroy_session(&self, session_id: AudioMusicSessionId) -> Result<(), String> {
        self.orchestration.destroy_music_session(session_id)
    }

    pub fn request_state(
        &self,
        session_id: AudioMusicSessionId,
        state: impl Into<String>,
    ) -> Result<(), String> {
        self.orchestration
            .request_music_state(session_id, state.into())
    }

    pub fn set_scalar(
        &self,
        session_id: AudioMusicSessionId,
        name: impl Into<String>,
        value: f32,
    ) -> Result<(), String> {
        self.orchestration
            .set_music_scalar(session_id, name.into(), value)
    }

    pub fn set_switch(
        &self,
        session_id: AudioMusicSessionId,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), String> {
        self.orchestration
            .set_music_switch(session_id, name.into(), value.into())
    }
}

#[allow(dead_code)]
fn _state_type_anchor(_: &InteractiveMusicRuntimeState) {}
