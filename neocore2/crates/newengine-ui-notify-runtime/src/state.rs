use std::collections::VecDeque;
use std::sync::Arc;

use newengine_game_events_api::GameMessageEnvelope;
use newengine_ui_api::{
    UiNotifyClearRequest, UiNotifyDismissRequest, UiNotifyMutationResponse, UiNotifyRequest,
    UiNotifySnapshotV1, UiToastNotification, UiToastSeverity, UiToastStack,
    UI_NOTIFY_ERROR_MESSAGE_ID, UI_NOTIFY_INFO_MESSAGE_ID, UI_NOTIFY_MESSAGE_ID,
    UI_NOTIFY_SUCCESS_MESSAGE_ID, UI_NOTIFY_WARNING_MESSAGE_ID,
};
use parking_lot::Mutex;

#[derive(Clone, Debug)]
pub struct UiNotifyPolicy {
    pub capacity: usize,
    pub visible_limit: usize,
    pub info_duration_ms: u64,
    pub success_duration_ms: u64,
    pub warning_duration_ms: u64,
    pub error_duration_ms: u64,
}

impl Default for UiNotifyPolicy {
    fn default() -> Self {
        Self {
            capacity: 64,
            visible_limit: 4,
            info_duration_ms: 4_500,
            success_duration_ms: 3_500,
            warning_duration_ms: 7_000,
            error_duration_ms: 9_000,
        }
    }
}

impl UiNotifyPolicy {
    fn normalized(mut self) -> Self {
        self.capacity = self.capacity.clamp(1, 4_096);
        self.visible_limit = self.visible_limit.clamp(1, self.capacity);
        self.info_duration_ms = self.info_duration_ms.clamp(250, 300_000);
        self.success_duration_ms = self.success_duration_ms.clamp(250, 300_000);
        self.warning_duration_ms = self.warning_duration_ms.clamp(250, 300_000);
        self.error_duration_ms = self.error_duration_ms.clamp(250, 300_000);
        self
    }

    fn duration_for(&self, severity: UiToastSeverity) -> u64 {
        match severity {
            UiToastSeverity::Info => self.info_duration_ms,
            UiToastSeverity::Success => self.success_duration_ms,
            UiToastSeverity::Warning => self.warning_duration_ms,
            UiToastSeverity::Error => self.error_duration_ms,
        }
    }
}

#[derive(Clone)]
pub struct UiNotifyRuntime {
    inner: Arc<Mutex<UiNotifyState>>,
}

struct ActiveToast {
    notification: UiToastNotification,
    sticky: bool,
    expires_at_ms: Option<u64>,
}

struct UiNotifyState {
    policy: UiNotifyPolicy,
    now_ms: u64,
    next_id: u64,
    generation: u64,
    dropped: u64,
    pipeline_dropped: u64,
    active: VecDeque<ActiveToast>,
}

impl Default for UiNotifyRuntime {
    fn default() -> Self {
        Self::new(UiNotifyPolicy::default())
    }
}

impl UiNotifyRuntime {
    pub fn new(policy: UiNotifyPolicy) -> Self {
        Self {
            inner: Arc::new(Mutex::new(UiNotifyState {
                policy: policy.normalized(),
                now_ms: 0,
                next_id: 0,
                generation: 1,
                dropped: 0,
                pipeline_dropped: 0,
                active: VecDeque::new(),
            })),
        }
    }

    pub fn push(&self, mut request: UiNotifyRequest) -> UiNotifyMutationResponse {
        let mut state = self.inner.lock();
        request.title = truncate_chars(request.title.trim(), 96);
        request.detail = truncate_chars(request.detail.trim(), 512);
        request.source = truncate_chars(request.source.trim(), 96);
        request.id = truncate_chars(request.id.trim(), 160);
        request.progress_permille = request.progress_permille.map(|value| value.min(1_000));

        if request.title.is_empty() && request.detail.is_empty() {
            return UiNotifyMutationResponse {
                queue_depth: state.active.len(),
                diagnostics: vec!["notification title and detail are both empty".to_owned()],
                ..Default::default()
            };
        }
        if request.source.is_empty() {
            request.source = "engine".to_owned();
        }
        if request.title.is_empty() {
            request.title = default_title(request.severity).to_owned();
        }

        state.next_id = state.next_id.wrapping_add(1).max(1);
        if request.id.is_empty() {
            request.id = request
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| truncate_chars(value, 160))
                .unwrap_or_else(|| format!("engine.ui.notify.{}", state.next_id));
        }

        if let Some(index) = state
            .active
            .iter()
            .position(|toast| toast.notification.id == request.id)
        {
            if request.replace_existing {
                state.active.remove(index);
            } else {
                request.id = format!("{}.{}", request.id, state.next_id);
            }
        }

        if state.active.len() >= state.policy.capacity {
            let removable = state.active.iter().rposition(|toast| !toast.sticky);
            if let Some(index) = removable {
                state.active.remove(index);
                state.dropped = state.dropped.saturating_add(1);
            } else {
                state.dropped = state.dropped.saturating_add(1);
                return UiNotifyMutationResponse {
                    id: request.id,
                    queue_depth: state.active.len(),
                    diagnostics: vec![
                        "notification queue is full and contains only sticky entries".to_owned(),
                    ],
                    ..Default::default()
                };
            }
        }

        let duration_ms = if request.duration_ms == 0 {
            state.policy.duration_for(request.severity)
        } else {
            request.duration_ms.clamp(250, 300_000)
        };
        let expires_at_ms = (!request.sticky).then(|| state.now_ms.saturating_add(duration_ms));
        let id = request.id.clone();
        state.active.push_front(ActiveToast {
            notification: UiToastNotification {
                id: request.id,
                title: request.title,
                detail: request.detail,
                progress_permille: request.progress_permille,
                severity: request.severity,
                source: request.source,
            },
            sticky: request.sticky,
            expires_at_ms,
        });
        state.generation = state.generation.wrapping_add(1).max(1);

        UiNotifyMutationResponse {
            accepted: true,
            affected: 1,
            id,
            queue_depth: state.active.len(),
            diagnostics: Vec::new(),
        }
    }

    pub fn dismiss(&self, request: UiNotifyDismissRequest) -> UiNotifyMutationResponse {
        let id = request.id.trim();
        let source = request
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut state = self.inner.lock();
        if id.is_empty() && source.is_none() {
            return UiNotifyMutationResponse {
                queue_depth: state.active.len(),
                diagnostics: vec!["dismiss requires a notification id or source filter".to_owned()],
                ..Default::default()
            };
        }
        let before = state.active.len();
        state.active.retain(|toast| {
            let id_matches = id.is_empty() || toast.notification.id == id;
            let source_matches = source.is_none_or(|source| toast.notification.source == source);
            !(id_matches && source_matches)
        });
        let affected = before.saturating_sub(state.active.len());
        if affected > 0 {
            state.generation = state.generation.wrapping_add(1).max(1);
        }
        UiNotifyMutationResponse {
            accepted: true,
            affected,
            id: id.to_owned(),
            queue_depth: state.active.len(),
            diagnostics: Vec::new(),
        }
    }

    pub fn clear(&self, request: UiNotifyClearRequest) -> UiNotifyMutationResponse {
        let source = request
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut state = self.inner.lock();
        let before = state.active.len();
        state.active.retain(|toast| {
            let source_matches = source.is_none_or(|source| toast.notification.source == source);
            !source_matches || (toast.sticky && !request.include_sticky)
        });
        let affected = before.saturating_sub(state.active.len());
        if affected > 0 {
            state.generation = state.generation.wrapping_add(1).max(1);
        }
        UiNotifyMutationResponse {
            accepted: true,
            affected,
            queue_depth: state.active.len(),
            diagnostics: Vec::new(),
            ..Default::default()
        }
    }

    pub fn ingest_message(
        &self,
        message: &GameMessageEnvelope,
    ) -> Option<UiNotifyMutationResponse> {
        request_from_game_message(message).map(|request| self.push(request))
    }

    pub fn advance(&self, dt_ms: u64) {
        let mut state = self.inner.lock();
        state.now_ms = state.now_ms.saturating_add(dt_ms.min(1_000));
        let now_ms = state.now_ms;
        let before = state.active.len();
        state.active.retain(|toast| {
            toast
                .expires_at_ms
                .is_none_or(|expires_at_ms| expires_at_ms > now_ms)
        });
        if state.active.len() != before {
            state.generation = state.generation.wrapping_add(1).max(1);
        }
    }

    pub fn observe_pipeline_dropped(&self, dropped: u64) {
        let mut state = self.inner.lock();
        state.pipeline_dropped = state.pipeline_dropped.max(dropped);
    }

    pub fn stack(&self, frame_index: u64) -> UiToastStack {
        let state = self.inner.lock();
        UiToastStack {
            version: state.generation as u32,
            frame_index,
            notifications: state
                .active
                .iter()
                .take(state.policy.visible_limit)
                .map(|toast| toast.notification.clone())
                .collect(),
        }
    }

    pub fn snapshot(&self) -> UiNotifySnapshotV1 {
        let state = self.inner.lock();
        UiNotifySnapshotV1 {
            version: 1,
            generation: state.generation,
            active: state.active.len(),
            visible_limit: state.policy.visible_limit,
            capacity: state.policy.capacity,
            dropped: state.dropped.saturating_add(state.pipeline_dropped),
            notifications: state
                .active
                .iter()
                .map(|toast| toast.notification.clone())
                .collect(),
        }
    }
}

pub fn request_from_game_message(message: &GameMessageEnvelope) -> Option<UiNotifyRequest> {
    let payload = &message.payload;
    let payload_severity = payload
        .get("severity")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_severity);
    let severity = if message.id == UI_NOTIFY_MESSAGE_ID {
        payload_severity.unwrap_or(UiToastSeverity::Info)
    } else {
        severity_from_message_id(&message.id)?
    };
    let title = payload
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| default_title(severity))
        .to_owned();
    let detail = ["detail", "message", "text"]
        .into_iter()
        .find_map(|key| payload.get(key).and_then(serde_json::Value::as_str))
        .or_else(|| payload.as_str())
        .unwrap_or_default()
        .to_owned();
    let id = payload
        .get("toast_id")
        .or_else(|| payload.get("notification_id"))
        .or_else(|| payload.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| message.correlation_id.clone())
        .unwrap_or_else(|| {
            if message.sequence > 0 {
                format!("{}.{}", message.id, message.sequence)
            } else {
                String::new()
            }
        });

    Some(UiNotifyRequest {
        id,
        title,
        detail,
        severity,
        source: if message.source.trim().is_empty() {
            "engine.game.events".to_owned()
        } else {
            message.source.clone()
        },
        duration_ms: payload
            .get("duration_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        sticky: payload
            .get("sticky")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        progress_permille: payload
            .get("progress_permille")
            .and_then(serde_json::Value::as_u64)
            .map(|value| value.min(1_000) as u16),
        replace_existing: payload
            .get("replace_existing")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        correlation_id: message.correlation_id.clone(),
    })
}

fn severity_from_message_id(id: &str) -> Option<UiToastSeverity> {
    match id {
        UI_NOTIFY_MESSAGE_ID | UI_NOTIFY_INFO_MESSAGE_ID => Some(UiToastSeverity::Info),
        UI_NOTIFY_SUCCESS_MESSAGE_ID => Some(UiToastSeverity::Success),
        UI_NOTIFY_WARNING_MESSAGE_ID => Some(UiToastSeverity::Warning),
        UI_NOTIFY_ERROR_MESSAGE_ID => Some(UiToastSeverity::Error),
        _ => None,
    }
}

fn parse_severity(value: &str) -> Option<UiToastSeverity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "info" | "message" | "notification" => Some(UiToastSeverity::Info),
        "success" | "ok" => Some(UiToastSeverity::Success),
        "warning" | "warn" => Some(UiToastSeverity::Warning),
        "error" | "danger" | "fatal" => Some(UiToastSeverity::Error),
        _ => None,
    }
}

fn default_title(severity: UiToastSeverity) -> &'static str {
    match severity {
        UiToastSeverity::Info => "Message",
        UiToastSeverity::Success => "Completed",
        UiToastSeverity::Warning => "Warning",
        UiToastSeverity::Error => "Error",
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_message_is_projected_into_a_toast() {
        let message = GameMessageEnvelope {
            id: UI_NOTIFY_WARNING_MESSAGE_ID.to_owned(),
            sequence: 7,
            source: "game.inventory".to_owned(),
            payload: serde_json::json!({
                "title": "Inventory",
                "message": "No free slots",
                "duration_ms": 5000
            }),
            ..Default::default()
        };
        let request = request_from_game_message(&message).expect("notify request");
        assert_eq!(request.severity, UiToastSeverity::Warning);
        assert_eq!(request.id, "engine.ui.notify.warning.7");
        assert_eq!(request.detail, "No free slots");
    }

    #[test]
    fn generic_message_honors_payload_severity() {
        let message = GameMessageEnvelope {
            id: UI_NOTIFY_MESSAGE_ID.to_owned(),
            payload: serde_json::json!({
                "title": "Thermals",
                "detail": "Temperature is above the warning threshold",
                "severity": "warning"
            }),
            ..Default::default()
        };
        let request = request_from_game_message(&message).expect("notify request");
        assert_eq!(request.severity, UiToastSeverity::Warning);
    }

    #[test]
    fn queue_is_bounded_and_replaces_stable_ids() {
        let runtime = UiNotifyRuntime::new(UiNotifyPolicy {
            capacity: 2,
            visible_limit: 2,
            ..Default::default()
        });
        for detail in ["first", "replacement"] {
            assert!(
                runtime
                    .push(UiNotifyRequest {
                        id: "stable".to_owned(),
                        title: "State".to_owned(),
                        detail: detail.to_owned(),
                        ..Default::default()
                    })
                    .accepted
            );
        }
        runtime.push(UiNotifyRequest {
            id: "second".to_owned(),
            title: "Second".to_owned(),
            ..Default::default()
        });
        runtime.push(UiNotifyRequest {
            id: "third".to_owned(),
            title: "Third".to_owned(),
            ..Default::default()
        });
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.active, 2);
        assert_eq!(snapshot.dropped, 1);
        assert_eq!(snapshot.notifications[1].id, "second");
    }

    #[test]
    fn empty_dismiss_request_is_rejected_without_clearing_the_queue() {
        let runtime = UiNotifyRuntime::default();
        runtime.push(UiNotifyRequest {
            title: "Keep me".to_owned(),
            ..Default::default()
        });
        let response = runtime.dismiss(UiNotifyDismissRequest::default());
        assert!(!response.accepted);
        assert_eq!(runtime.snapshot().active, 1);
    }

    #[test]
    fn transient_entries_expire_while_sticky_entries_survive() {
        let runtime = UiNotifyRuntime::default();
        runtime.push(UiNotifyRequest {
            id: "transient".to_owned(),
            title: "Transient".to_owned(),
            duration_ms: 250,
            ..Default::default()
        });
        runtime.push(UiNotifyRequest {
            id: "sticky".to_owned(),
            title: "Sticky".to_owned(),
            sticky: true,
            ..Default::default()
        });
        runtime.advance(251);
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.active, 1);
        assert_eq!(snapshot.notifications[0].id, "sticky");
    }
}
