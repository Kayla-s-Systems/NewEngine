#[derive(Debug, Clone, Default)]
pub(super) struct LoadSelection {
    pub(super) bootstrap_candidates: Vec<PathBuf>,
    pub(super) engine_candidates: Vec<PathBuf>,
    pub(super) decisions: HashMap<PathBuf, SelectionDecision>,
}

#[derive(Debug, Clone)]
pub(super) enum SelectionDecision {
    Selected,
    Runtime { label: &'static str },
    Unsupported { reason: &'static str },
    Unknown,
    AlreadyLoaded,
    DisabledByConfig,
    Filtered { filter_label: &'static str },
    DuplicateId { winner_file: String },
}

impl SelectionDecision {
    #[inline]
    pub(super) fn is_selected(&self) -> bool {
        matches!(self, Self::Selected)
    }

    #[inline]
    pub(super) fn is_runtime(&self) -> bool {
        matches!(self, Self::Runtime { .. })
    }

    #[inline]
    pub(super) fn is_duplicate(&self) -> bool {
        matches!(self, Self::DuplicateId { .. })
    }

    #[inline]
    pub(super) fn selected_label(&self) -> &'static str {
        match self {
            Self::Selected => "yes",
            Self::Runtime { .. } => "runtime",
            Self::Unsupported { .. }
            | Self::Unknown
            | Self::AlreadyLoaded
            | Self::DisabledByConfig
            | Self::Filtered { .. }
            | Self::DuplicateId { .. } => "no",
        }
    }

    #[inline]
    pub(super) fn reason_label(&self) -> String {
        match self {
            Self::Selected => "phase match".to_owned(),
            Self::Runtime { label } => format!("{label} runtime"),
            Self::Unsupported { reason } => (*reason).to_owned(),
            Self::Unknown => "unknown dynlib".to_owned(),
            Self::AlreadyLoaded => "already loaded".to_owned(),
            Self::DisabledByConfig => "disabled by config".to_owned(),
            Self::Filtered { filter_label } => format!("filtered by {filter_label}"),
            Self::DuplicateId { winner_file } => {
                format!("duplicate plugin id, winner='{winner_file}'")
            }
        }
    }
}

struct HostPluginPolicySnapshot {
    excluded_ids: NeHashSet<String>,
    enabled_by_id: HashMap<String, bool>,
}

impl HostPluginPolicySnapshot {
    fn capture() -> Self {
        let mut excluded_ids = NeHashSet::default();
        if let Some(value) = crate::host_context::environment_var("NEWENGINE_PLUGIN_EXCLUDE_IDS") {
            excluded_ids.extend(
                value
                    .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_owned),
            );
        }
        Self {
            excluded_ids,
            enabled_by_id: HashMap::default(),
        }
    }

    #[inline]
    fn is_excluded(&self, id: &str) -> bool {
        self.excluded_ids.contains(id)
    }

    fn is_enabled(&mut self, id: &str) -> bool {
        if let Some(enabled) = self.enabled_by_id.get(id) {
            return *enabled;
        }
        let enabled = crate::plugin_config_service::plugin_enabled_by_config(id);
        self.enabled_by_id.insert(id.to_owned(), enabled);
        enabled
    }
}
