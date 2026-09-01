use std::sync::{Arc, Mutex, OnceLock};

/// Immutable decoded animation dictionary. Clips are independently reference-counted so runtime
/// bindings can retain selected clips while the dictionary cache is invalidated for hot reload.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationDictionary {
    pub clips: Vec<Arc<AnimationClip>>,
}

impl AnimationDictionary {
    #[inline]
    pub fn clip(&self, selector: Option<&str>) -> Option<Arc<AnimationClip>> {
        let requested = selector.map(str::trim).filter(|value| !value.is_empty());
        match requested {
            Some(name) => self
                .clips
                .iter()
                .find(|clip| clip.name.eq_ignore_ascii_case(name))
                .cloned(),
            None => self.clips.first().cloned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationClipReference {
    pub logical_path: String,
    pub selector: Option<String>,
    pub canonical_path_key: String,
    pub canonical_clip_key: String,
}

impl AnimationClipReference {
    pub fn parse(reference: &str) -> Result<Self, String> {
        let normalized = reference.trim().replace('\\', "/");
        if normalized.is_empty() {
            return Err("animation clip reference is empty".to_owned());
        }
        let (path, selector) = normalized
            .rsplit_once('@')
            .map(|(path, selector)| {
                let selector = selector.trim();
                (
                    path.trim().trim_start_matches('/').to_owned(),
                    (!selector.is_empty()).then(|| selector.to_owned()),
                )
            })
            .unwrap_or_else(|| (normalized.trim_start_matches('/').to_owned(), None));
        if path.is_empty() {
            return Err(format!(
                "animation clip reference has no logical path ref='{reference}'"
            ));
        }
        let canonical_path_key = path.to_ascii_lowercase();
        let canonical_clip_key = selector.as_ref().map_or_else(
            || canonical_path_key.clone(),
            |selector| format!("{}@{}", canonical_path_key, selector.to_ascii_lowercase()),
        );
        Ok(Self {
            logical_path: path,
            selector,
            canonical_path_key,
            canonical_clip_key,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationClipStoreStats {
    pub dictionaries: usize,
    pub clips: usize,
}

/// Process-shared immutable YCD dictionary/clip cache.
///
/// Asset I/O happens outside the mutex. Concurrent cold misses may decode the same dictionary, but
/// insertion converges on one authoritative `Arc<AnimationDictionary>` and all retained clip handles
/// are immutable `Arc<AnimationClip>` values.
#[derive(Debug, Default)]
pub struct AnimationClipStore {
    dictionaries: Mutex<std::collections::HashMap<String, Arc<AnimationDictionary>>>,
    /// Selected-entry fallback cache for dictionaries that fail strict whole-dictionary decode
    /// because an unrelated clip is malformed. Keys are canonical `path@selector` identities.
    isolated_clips: Mutex<std::collections::HashMap<String, Arc<AnimationClip>>>,
}

impl AnimationClipStore {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn cached_dictionary(
        &self,
        canonical_path_key: &str,
    ) -> Result<Option<Arc<AnimationDictionary>>, String> {
        self.dictionaries
            .lock()
            .map(|guard| guard.get(canonical_path_key).cloned())
            .map_err(|_| "animation clip store mutex poisoned".to_owned())
    }

    fn cached_isolated_clip(
        &self,
        canonical_clip_key: &str,
    ) -> Result<Option<Arc<AnimationClip>>, String> {
        self.isolated_clips
            .lock()
            .map(|guard| guard.get(canonical_clip_key).cloned())
            .map_err(|_| "animation clip store isolated cache mutex poisoned".to_owned())
    }

    pub fn load_ycd_dictionary<F>(
        &self,
        logical_path: &str,
        load_body: F,
    ) -> Result<Arc<AnimationDictionary>, String>
    where
        F: FnOnce(&str) -> Result<Vec<u8>, String>,
    {
        let parsed = AnimationClipReference::parse(logical_path)?;
        if parsed.selector.is_some() {
            return Err(format!(
                "animation dictionary load requires path without selector path='{logical_path}'"
            ));
        }
        if let Some(cached) = self.cached_dictionary(&parsed.canonical_path_key)? {
            return Ok(cached);
        }

        let body = load_body(&parsed.logical_path)?;
        let decoded = Arc::new(decode_ycd_dictionary(&body)?);
        let mut guard = self
            .dictionaries
            .lock()
            .map_err(|_| "animation clip store mutex poisoned".to_owned())?;
        Ok(guard
            .entry(parsed.canonical_path_key)
            .or_insert_with(|| decoded.clone())
            .clone())
    }

    pub fn load_ycd_clip<F>(
        &self,
        reference: &str,
        load_body: F,
    ) -> Result<Arc<AnimationClip>, String>
    where
        F: FnOnce(&str) -> Result<Vec<u8>, String>,
    {
        let parsed = AnimationClipReference::parse(reference)?;
        if let Some(cached) = self.cached_dictionary(&parsed.canonical_path_key)? {
            return cached.clip(parsed.selector.as_deref()).ok_or_else(|| {
                format!(
                    "animation selector '{}' was not found in dictionary '{}'",
                    parsed.selector.as_deref().unwrap_or("<first>"),
                    parsed.logical_path
                )
            });
        }
        if parsed.selector.is_some() {
            if let Some(cached) = self.cached_isolated_clip(&parsed.canonical_clip_key)? {
                return Ok(cached);
            }
        }

        let body = load_body(&parsed.logical_path)?;
        match decode_ycd_dictionary(&body) {
            Ok(dictionary) => {
                let decoded = Arc::new(dictionary);
                let selected = decoded.clip(parsed.selector.as_deref()).ok_or_else(|| {
                    format!(
                        "animation selector '{}' was not found in dictionary '{}'",
                        parsed.selector.as_deref().unwrap_or("<first>"),
                        parsed.logical_path
                    )
                })?;
                let mut guard = self
                    .dictionaries
                    .lock()
                    .map_err(|_| "animation clip store mutex poisoned".to_owned())?;
                guard
                    .entry(parsed.canonical_path_key)
                    .or_insert_with(|| decoded);
                Ok(selected)
            }
            Err(dictionary_error) => {
                let Some(selector) = parsed.selector.as_deref() else {
                    return Err(dictionary_error);
                };
                let selected = Arc::new(decode_ycd_body(&body, Some(selector)).map_err(
                    |selected_error| {
                        format!(
                            "strict YCD dictionary decode failed: {dictionary_error}; selected-entry decode failed: {selected_error}"
                        )
                    },
                )?);
                let mut guard = self
                    .isolated_clips
                    .lock()
                    .map_err(|_| "animation clip store isolated cache mutex poisoned".to_owned())?;
                Ok(guard
                    .entry(parsed.canonical_clip_key)
                    .or_insert_with(|| selected.clone())
                    .clone())
            }
        }
    }

    /// Installs a new immutable event-track revision for a cached clip.
    ///
    /// Existing actor bindings keep their previous `Arc<AnimationClip>` revision. Future lookups
    /// resolve the newly installed dictionary revision. This is the editor/importer hot-reload path
    /// until canonical YCD binary authoring grows a persisted event table.
    pub fn install_clip_events(
        &self,
        reference: &str,
        events: Vec<AnimationEvent>,
    ) -> Result<Arc<AnimationClip>, String> {
        let parsed = AnimationClipReference::parse(reference)?;
        let mut guard = self
            .dictionaries
            .lock()
            .map_err(|_| "animation clip store mutex poisoned".to_owned())?;
        let dictionary = guard
            .get(&parsed.canonical_path_key)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "animation dictionary is not loaded for event install path='{}'",
                    parsed.logical_path
                )
            })?;
        let selected_index = match parsed.selector.as_deref() {
            Some(selector) => dictionary
                .clips
                .iter()
                .position(|clip| clip.name.eq_ignore_ascii_case(selector)),
            None => (!dictionary.clips.is_empty()).then_some(0),
        }
        .ok_or_else(|| {
            format!(
                "animation selector '{}' was not found for event install path='{}'",
                parsed.selector.as_deref().unwrap_or("<first>"),
                parsed.logical_path
            )
        })?;

        let mut revised_clip = (*dictionary.clips[selected_index]).clone();
        revised_clip.events = events;
        revised_clip.validate_structure()?;
        let revised_clip = Arc::new(revised_clip);
        let mut revised_clips = dictionary.clips.clone();
        revised_clips[selected_index] = revised_clip.clone();
        guard.insert(
            parsed.canonical_path_key,
            Arc::new(AnimationDictionary {
                clips: revised_clips,
            }),
        );
        Ok(revised_clip)
    }

    /// Invalidates future lookups without invalidating already-bound actors.
    pub fn invalidate_ycd_path(&self, logical_path: &str) -> Result<bool, String> {
        let parsed = AnimationClipReference::parse(logical_path)?;
        let dictionary_removed = self
            .dictionaries
            .lock()
            .map_err(|_| "animation clip store mutex poisoned".to_owned())?
            .remove(&parsed.canonical_path_key)
            .is_some();
        let isolated_prefix = format!("{}@", parsed.canonical_path_key);
        let mut isolated = self
            .isolated_clips
            .lock()
            .map_err(|_| "animation clip store isolated cache mutex poisoned".to_owned())?;
        let before = isolated.len();
        isolated.retain(|key, _| !key.starts_with(&isolated_prefix));
        Ok(dictionary_removed || isolated.len() != before)
    }

    pub fn clear(&self) -> Result<(), String> {
        self.dictionaries
            .lock()
            .map_err(|_| "animation clip store mutex poisoned".to_owned())?
            .clear();
        self.isolated_clips
            .lock()
            .map_err(|_| "animation clip store isolated cache mutex poisoned".to_owned())?
            .clear();
        Ok(())
    }

    pub fn stats(&self) -> Result<AnimationClipStoreStats, String> {
        let dictionaries = self
            .dictionaries
            .lock()
            .map_err(|_| "animation clip store mutex poisoned".to_owned())?;
        let isolated = self
            .isolated_clips
            .lock()
            .map_err(|_| "animation clip store isolated cache mutex poisoned".to_owned())?;
        Ok(AnimationClipStoreStats {
            dictionaries: dictionaries.len(),
            clips: dictionaries
                .values()
                .map(|dictionary| dictionary.clips.len())
                .sum::<usize>()
                + isolated.len(),
        })
    }
}

pub fn global_animation_clip_store() -> &'static AnimationClipStore {
    static STORE: OnceLock<AnimationClipStore> = OnceLock::new();
    STORE.get_or_init(AnimationClipStore::new)
}
