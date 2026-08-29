use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::normalization::normalize_optional_string;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeActionRoute {
    pub id: String,
    pub source: String,
    pub target: String,
    pub event: String,
    #[serde(default)]
    pub payload: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub transition: Option<UiNodeTransition>,
    #[serde(default)]
    pub feedback: Option<UiNodeFeedbackEvent>,
    #[serde(default)]
    pub audio: Option<String>,
}

impl UiNodeActionRoute {
    pub(crate) fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.source = self.source.trim().to_owned();
        self.target = self.target.trim().to_owned();
        self.event = self.event.trim().to_owned();
        self.audio = self
            .audio
            .take()
            .and_then(|value| normalize_optional_string(&value));
        if let Some(transition) = &mut self.transition {
            transition.canonicalize_in_place();
        }
        if let Some(feedback) = &mut self.feedback {
            feedback.canonicalize_in_place();
        }
    }

    #[inline]
    pub fn payload_str(&self, key: &str) -> Option<&str> {
        self.payload.get(key).and_then(serde_json::Value::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeTransitionKind {
    None,
    OpenPage,
    Back,
    Close,
}

impl Default for UiNodeTransitionKind {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNodeTransition {
    #[serde(default)]
    pub kind: UiNodeTransitionKind,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default = "default_true")]
    pub reset_selection: bool,
}

impl UiNodeTransition {
    #[inline]
    pub fn close() -> Self {
        Self {
            kind: UiNodeTransitionKind::Close,
            page: None,
            reset_selection: true,
        }
    }

    #[inline]
    pub fn open_page(page: impl Into<String>) -> Self {
        Self {
            kind: UiNodeTransitionKind::OpenPage,
            page: Some(page.into()),
            reset_selection: true,
        }
    }

    pub(crate) fn canonicalize_in_place(&mut self) {
        self.page = self
            .page
            .take()
            .and_then(|value| normalize_optional_string(&value));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeFeedbackSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for UiNodeFeedbackSeverity {
    #[inline]
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeFeedbackEvent {
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub severity: UiNodeFeedbackSeverity,
    #[serde(default = "default_feedback_ttl_sec")]
    pub ttl_sec: f32,
}

impl UiNodeFeedbackEvent {
    #[inline]
    pub fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiNodeFeedbackSeverity,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            severity,
            ttl_sec: default_feedback_ttl_sec(),
        }
    }

    pub(crate) fn canonicalize_in_place(&mut self) {
        self.title = self.title.trim().to_owned();
        self.detail = self.detail.trim().to_owned();
        self.ttl_sec = self.ttl_sec.clamp(0.05, 30.0);
    }
}

#[inline]
fn default_true() -> bool {
    true
}

#[inline]
fn default_feedback_ttl_sec() -> f32 {
    2.25
}
