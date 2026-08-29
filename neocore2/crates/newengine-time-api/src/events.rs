use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeScheduledEventV1 {
    pub id: String,
    pub due_simulation_tick: Option<u64>,
    pub due_monotonic_ns: Option<u64>,
    pub payload_json: serde_json::Value,
}

impl Default for TimeScheduledEventV1 {
    fn default() -> Self {
        Self {
            id: String::new(),
            due_simulation_tick: None,
            due_monotonic_ns: None,
            payload_json: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeCancelEventRequestV1 {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeDueEventsV1 {
    pub events: Vec<TimeScheduledEventV1>,
}
