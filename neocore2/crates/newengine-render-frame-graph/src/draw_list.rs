use newengine_render_api::RenderDrawListKind;
use serde::{Deserialize, Serialize};

pub type DrawListKind = RenderDrawListKind;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawListStats {
    pub draw_calls: u32,
    pub indexed_draw_calls: u32,
    pub triangle_count: u64,
    pub instance_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawListDesc {
    pub kind: DrawListKind,
    pub label: String,
    #[serde(default)]
    pub stats: DrawListStats,
}

impl DrawListDesc {
    #[inline]
    pub fn new(kind: DrawListKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            stats: DrawListStats::default(),
        }
    }

    #[inline]
    pub fn standard(kind: DrawListKind) -> Self {
        Self::new(kind, kind.label())
    }

    #[inline]
    pub fn with_stats(mut self, stats: DrawListStats) -> Self {
        self.stats = stats;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawListSetDesc {
    #[serde(default)]
    pub lists: Vec<DrawListDesc>,
}

impl DrawListSetDesc {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn push(&mut self, list: DrawListDesc) {
        if let Some(existing) = self.lists.iter_mut().find(|it| it.kind == list.kind) {
            *existing = list;
        } else {
            self.lists.push(list);
        }
    }

    #[inline]
    pub fn with(mut self, list: DrawListDesc) -> Self {
        self.push(list);
        self
    }

    #[inline]
    pub fn contains(&self, kind: DrawListKind) -> bool {
        self.lists.iter().any(|it| it.kind == kind)
    }

    #[inline]
    pub fn into_vec(self) -> Vec<DrawListDesc> {
        self.lists
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawListRouteValidationIssue {
    pub code: String,
    pub message: String,
    pub draw_list: Option<DrawListKind>,
}

impl DrawListRouteValidationIssue {
    #[inline]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            draw_list: None,
        }
    }

    #[inline]
    pub fn with_draw_list(mut self, draw_list: DrawListKind) -> Self {
        self.draw_list = Some(draw_list);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawListRouteValidationReport {
    pub ok: bool,
    pub errors: Vec<DrawListRouteValidationIssue>,
    pub warnings: Vec<DrawListRouteValidationIssue>,
}
