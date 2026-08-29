use serde::{Deserialize, Serialize};

pub const ENGINE_TIME_SERVICE_ID: &str = "engine.time";
pub const TIME_SERVICE_ID: &str = "time.api";
pub const TIME_BACKEND_CAPABILITY_ID: &str = "time.backend";
pub const TIME_RUNTIME_CONTRACT: &str = "newengine.time.runtime.v1";

pub mod time_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const SNAPSHOT_V1: &str = "time.snapshot_v1";
    pub const BEGIN_FRAME_V1: &str = "time.begin_frame_v1";
    pub const ADVANCE_FIXED_V1: &str = "time.advance_fixed_v1";
    pub const FRAME_V1: &str = "time.frame_v1";
    pub const FIXED_TICK_V1: &str = "time.fixed_tick_v1";
    pub const GAME_CLOCK_V1: &str = "time.game_clock_v1";
    pub const PAUSE_DOMAIN_V1: &str = "time.pause_domain_v1";
    pub const TIMELINE_V1: &str = "time.timeline_v1";
    pub const REPLAY_CLOCK_V1: &str = "time.replay_clock_v1";
    pub const SET_SCALE_V1: &str = "time.set_scale_v1";
    pub const SET_PAUSE_V1: &str = "time.set_pause_v1";
    pub const SET_GAME_CLOCK_V1: &str = "time.set_game_clock_v1";
    pub const SCHEDULE_EVENT_V1: &str = "time.schedule_event_v1";
    pub const CANCEL_EVENT_V1: &str = "time.cancel_event_v1";
    pub const DUE_EVENTS_V1: &str = "time.due_events_v1";
    pub const DESCRIBE_CLOCK_V1: &str = "time.describe_clock_v1";
    pub const AI_CONTEXT_V1: &str = "time.ai_context_v1";
    pub const SET_FIXED_STEP_V1: &str = "time.set_fixed_step_v1";
    pub const SET_REPLAY_CLOCK_V1: &str = "time.set_replay_clock_v1";
}

pub const TIME_SERVICE_METHODS: &[&str] = &[
    time_method::INFO_JSON,
    time_method::INVOKE_JSON,
    time_method::SHUTDOWN_V1,
    time_method::SNAPSHOT_V1,
    time_method::BEGIN_FRAME_V1,
    time_method::ADVANCE_FIXED_V1,
    time_method::FRAME_V1,
    time_method::FIXED_TICK_V1,
    time_method::GAME_CLOCK_V1,
    time_method::PAUSE_DOMAIN_V1,
    time_method::TIMELINE_V1,
    time_method::REPLAY_CLOCK_V1,
    time_method::SET_SCALE_V1,
    time_method::SET_PAUSE_V1,
    time_method::SET_GAME_CLOCK_V1,
    time_method::SCHEDULE_EVENT_V1,
    time_method::CANCEL_EVENT_V1,
    time_method::DUE_EVENTS_V1,
    time_method::DESCRIBE_CLOCK_V1,
    time_method::AI_CONTEXT_V1,
    time_method::SET_FIXED_STEP_V1,
    time_method::SET_REPLAY_CLOCK_V1,
];

pub const TIME_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "time",
        ENGINE_TIME_SERVICE_ID,
        TIME_SERVICE_ID,
        TIME_BACKEND_CAPABILITY_ID,
    );

pub const TIME_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_TIME_SERVICE_ID,
        TIME_RUNTIME_CONTRACT,
        TIME_SERVICE_METHODS,
    );

pub const TIME_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        TIME_RUNTIME_CONTRACT_SPEC,
        Some(TIME_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_TIME_BACKEND"),
    );

const TIME_CLOCK_DOMAINS: &[&str] = &["real", "simulation", "game", "replay", "ai"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeServiceInfoV1 {
    pub service_id: String,
    pub gateway: String,
    pub provider: String,
    pub contract: String,
    pub methods: Vec<String>,
    pub deterministic: bool,
    pub ai_ready: bool,
    pub clock_domains: Vec<String>,
}

impl Default for TimeServiceInfoV1 {
    fn default() -> Self {
        Self {
            service_id: TIME_SERVICE_ID.to_owned(),
            gateway: ENGINE_TIME_SERVICE_ID.to_owned(),
            provider: "AstrolabeTimeProvider".to_owned(),
            contract: TIME_RUNTIME_CONTRACT.to_owned(),
            methods: TIME_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            deterministic: false,
            ai_ready: true,
            clock_domains: TIME_CLOCK_DOMAINS
                .iter()
                .map(|domain| (*domain).to_owned())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_contract_is_gateway_first() {
        assert_eq!(ENGINE_TIME_SERVICE_ID, "engine.time");
        assert_eq!(
            TIME_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_TIME_SERVICE_ID
        );
        assert_eq!(
            TimeServiceInfoV1::default().methods.len(),
            TIME_SERVICE_METHODS.len()
        );
    }
}
