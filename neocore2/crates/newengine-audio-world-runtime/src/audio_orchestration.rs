use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use newengine_audio_api::{
    AudioInstanceId, AudioMixGraph, AudioMusicSessionId, AudioMusicSessionState, AudioObjectId,
    AudioObjectState, AudioOrchestrationCommand, AudioParameterSet, AudioParameterTarget,
    AudioPlayInstanceRequest, AudioPlayStreamInstanceRequest, AudioRenderClock,
    AudioRouteGainRequest, AudioRouteId, AudioTransportAction, AudioTransportActionId,
    AudioTransportConfig, AudioTransportInstanceState, AudioTransportMarkerOccurrence,
    AudioTransportRuntimeState, AudioTransportSchedulePoint, AudioVoiceBudgetConfig,
    AudioVoiceRenderAction, AudioVoiceRenderScheduleRequest, AudioVoiceUpdateRequest,
    InteractiveMusicGraph, InteractiveMusicRuntimeState,
};
use newengine_audio_client::{
    audio_render_clock, play_audio_cue, play_audio_stream, schedule_audio_voice_render,
    set_audio_route_gain, set_audio_voice_budgets, stop_audio_voice, update_audio_voice,
};
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::audio_transport::{
    AudioTransportHandle, AudioTransportRuntime, DueTransportAction, PendingTransportAction,
};
use crate::interactive_music::InteractiveMusicHandle;

mod config;
mod handle;
mod music;
mod transition;

include!("audio_orchestration/transport_runtime.rs");
include!("audio_orchestration/mix_runtime.rs");

pub use config::AudioOrchestrationRuntimeConfig;
pub use handle::AudioOrchestrationHandle;
use transition::SampleTransition;

include!("audio_orchestration/state.rs");
include!("audio_orchestration/commands.rs");
include!("audio_orchestration/play.rs");
include!("audio_orchestration/snapshot.rs");
include!("audio_orchestration/module.rs");
include!("audio_orchestration/util.rs");

#[cfg(test)]
mod tests;
