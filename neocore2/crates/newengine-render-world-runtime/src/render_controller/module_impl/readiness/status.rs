#[derive(Clone, Debug)]
pub(super) struct LaunchReadiness {
    pub ready: bool,
    pub reason: String,
    pub waiting: u32,
    pub total: u32,
    pub failed: u32,
}

impl LaunchReadiness {
    #[inline]
    pub fn ready(reason: impl Into<String>, total: u32, failed: u32) -> Self {
        Self {
            ready: true,
            reason: reason.into(),
            waiting: 0,
            total,
            failed,
        }
    }

    #[inline]
    pub fn pending(reason: impl Into<String>, waiting: u32, total: u32, failed: u32) -> Self {
        Self {
            ready: false,
            reason: reason.into(),
            waiting,
            total,
            failed,
        }
    }

    pub fn aggregate(parts: &[Self]) -> Self {
        let ready = parts.iter().all(|part| part.ready);
        let reason = if ready {
            parts
                .iter()
                .map(|part| part.reason.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            parts
                .iter()
                .find(|part| !part.ready)
                .map(|part| part.reason.clone())
                .unwrap_or_else(|| "launch residency pending".to_owned())
        };
        Self {
            ready,
            reason,
            waiting: parts
                .iter()
                .fold(0_u32, |acc, part| acc.saturating_add(part.waiting)),
            total: parts
                .iter()
                .fold(0_u32, |acc, part| acc.saturating_add(part.total)),
            failed: parts
                .iter()
                .fold(0_u32, |acc, part| acc.saturating_add(part.failed)),
        }
    }
}
