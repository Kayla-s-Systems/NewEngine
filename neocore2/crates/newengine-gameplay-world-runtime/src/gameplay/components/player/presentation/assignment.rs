/// Desired player avatar assignment. Game/editor code changes this component;
/// the active world package resolves it to a concrete runtime model binding.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerModelAssignment {
    pub revision: u64,
    pub enabled: bool,
    pub source: String,
    pub properties_ref: Option<String>,
    pub texture_dictionary: Option<String>,
    pub skeleton_source: Option<String>,
    /// Authoritative project-owned animation bindings. Runtime may request a semantic capability
    /// id, but it never assumes filename, directory, extension, or clip name.
    pub animation_slots: std::collections::BTreeMap<String, String>,
    /// Legacy compatibility field. New project data should use `animation_slots`.
    /// Semantic idle clip reference, e.g. `animations/foo.ycd@idle`.
    pub idle_animation: Option<String>,
    pub walk_animation: Option<String>,
    pub run_animation: Option<String>,
    pub sprint_animation: Option<String>,
    pub crouch_idle_animation: Option<String>,
    pub crouch_walk_animation: Option<String>,
    pub jump_animation: Option<String>,
    pub fall_animation: Option<String>,
    pub presentation: PlayerCharacterPresentation,
    pub target_height: f32,
    pub eye_height_ratio: f32,
    pub local_offset: Vec3,
    pub yaw_offset: f32,
    /// Legacy compatibility metadata. Game-ready full-body FPP does not hide the whole avatar.
    pub hide_in_first_person: bool,
}

impl PlayerModelAssignment {
    #[inline]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            revision: 1,
            enabled: true,
            source: source.into(),
            ..Self::default()
        }
    }

    /// Resolve an authored animation capability without any path/name convention.
    #[inline]
    pub fn animation_for_slot(&self, slot: &str) -> Option<&str> {
        self.animation_slots.get(slot).map(String::as_str)
    }

    #[inline]
    pub fn next_revision_after(mut self, previous: Option<&Self>) -> Self {
        self.revision = previous
            .map(|assignment| assignment.revision.saturating_add(1).max(1))
            .unwrap_or_else(|| self.revision.max(1));
        self
    }
}

impl Default for PlayerModelAssignment {
    #[inline]
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: false,
            source: String::new(),
            properties_ref: None,
            texture_dictionary: None,
            skeleton_source: None,
            animation_slots: std::collections::BTreeMap::new(),
            idle_animation: None,
            walk_animation: None,
            run_animation: None,
            sprint_animation: None,
            crouch_idle_animation: None,
            crouch_walk_animation: None,
            jump_animation: None,
            fall_animation: None,
            presentation: PlayerCharacterPresentation::default(),
            target_height: 1.80,
            eye_height_ratio: 0.91,
            local_offset: Vec3::ZERO,
            yaw_offset: 0.0,
            hide_in_first_person: false,
        }
    }
}
