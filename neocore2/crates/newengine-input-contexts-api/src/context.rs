use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputCapturePolicy {
    ObserveOnly,
    ConsumeMatched,
    ConsumeAll,
}

impl Default for InputCapturePolicy {
    #[inline]
    fn default() -> Self {
        Self::ObserveOnly
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputContextLifetime {
    Frame,
    Modal,
    Persistent,
}

impl Default for InputContextLifetime {
    #[inline]
    fn default() -> Self {
        Self::Modal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputContext {
    pub id: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub capture: InputCapturePolicy,
    #[serde(default)]
    pub lifetime: InputContextLifetime,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl InputContext {
    #[inline]
    pub fn new(id: impl Into<String>, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            priority: 0,
            capture: InputCapturePolicy::ObserveOnly,
            lifetime: InputContextLifetime::Modal,
            enabled: true,
        }
    }

    #[inline]
    pub fn consuming(mut self) -> Self {
        self.capture = InputCapturePolicy::ConsumeMatched;
        self
    }

    #[inline]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputContextStack {
    #[serde(default)]
    pub contexts: Vec<InputContext>,
}

impl InputContextStack {
    pub fn canonicalized(mut self) -> Self {
        self.contexts.retain(|ctx| !ctx.id.trim().is_empty());
        self.contexts
            .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        self
    }
}

#[inline]
fn default_true() -> bool {
    true
}
