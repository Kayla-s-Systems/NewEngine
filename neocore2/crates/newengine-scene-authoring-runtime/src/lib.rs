#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use newengine_ecs::{EntityId, World};
use newengine_math::{EulerRot, Vec3};
pub use newengine_scene_authoring_api::AuthoredProjectEditStatus;
use newengine_scene_authoring_api::SceneAuthoringService;
use newengine_transform::Transform;
use newengine_world_authoring_api::{
    AuthoredMapPlacement, AuthoredMapPlacementCloneSource, AuthoredMapPlacementDirty,
    AuthoredMapPlacementSource,
};
use parking_lot::Mutex;

#[derive(Clone, Debug)]
struct AuthoredPlacementEdit {
    entity: EntityId,
    map_ref: String,
    placement_id: String,
    source: AuthoredMapPlacementSource,
    transform: Transform,
    clone_source_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AuthoredPlacementKey {
    map_ref: String,
    placement_id: String,
    source_kind: u8,
}

impl AuthoredPlacementKey {
    fn new(
        map_ref: impl Into<String>,
        placement_id: impl Into<String>,
        source: AuthoredMapPlacementSource,
    ) -> Self {
        Self {
            map_ref: map_ref.into(),
            placement_id: placement_id.into(),
            source_kind: authored_source_key(source),
        }
    }

    fn from_authored(authored: &AuthoredMapPlacement) -> Self {
        Self::new(
            authored.map_ref.clone(),
            authored.placement_id.clone(),
            authored.source,
        )
    }

    fn from_edit(edit: &AuthoredPlacementEdit) -> Self {
        Self::new(edit.map_ref.clone(), edit.placement_id.clone(), edit.source)
    }

    fn source(&self) -> AuthoredMapPlacementSource {
        authored_source_from_key(self.source_kind)
    }
}

#[derive(Clone, Debug, Default)]
struct AuthoredMapEditJournal {
    deletes: BTreeSet<AuthoredPlacementKey>,
    next_copy_sequence: u64,
    last_save_succeeded: Option<bool>,
    last_save_message: String,
}

#[derive(Debug, Default)]
pub struct SceneAuthoringRuntime {
    journal: Mutex<AuthoredMapEditJournal>,
    in_game_editor_enabled: Mutex<bool>,
}

impl SceneAuthoringRuntime {
    #[inline]
    pub fn in_game_editor_enabled(&self) -> bool {
        *self.in_game_editor_enabled.lock()
    }

    /// Updates only authoring-session state. Product/UI side effects belong to the
    /// composition adapter that consumes this focused runtime.
    pub fn set_in_game_editor_enabled(&self, enabled: bool) -> bool {
        let changed = {
            let mut current = self.in_game_editor_enabled.lock();
            if *current == enabled {
                false
            } else {
                *current = enabled;
                true
            }
        };
        changed
    }

    /// Persists all authored in-game editor changes to the current project's YMAP
    /// source and rebuilds the compiled NEF8 YMAP through the package-writer gateway.
    ///
    /// Transform edits, actor duplicates and actor deletions share one transaction
    /// boundary. Runtime EntityId values never cross it; save-back is keyed only by
    /// AuthoredMapPlacement identity.
    pub fn save_authored_project_world(
        &self,
        world: &mut World,
        project_root: Option<&Path>,
    ) -> Result<usize, String> {
        let result = self.save_authored_project_world_inner(world, project_root);
        let mut journal = self.journal.lock();
        match &result {
            Ok(0) => {
                journal.last_save_succeeded = Some(true);
                journal.last_save_message = "No authored map changes to save".to_owned();
            }
            Ok(count) => {
                journal.last_save_succeeded = Some(true);
                journal.last_save_message = format!("Saved {count} authored map change(s)");
            }
            Err(error) => {
                journal.last_save_succeeded = Some(false);
                journal.last_save_message = error.clone();
            }
        }
        result
    }

    fn save_authored_project_world_inner(
        &self,
        world: &mut World,
        project_root: Option<&Path>,
    ) -> Result<usize, String> {
        let edits = self.collect_authored_placement_edits(world)?;
        let deletes = self.journal.lock().deletes.clone();
        let creates = edits
            .iter()
            .filter_map(|edit| {
                edit.clone_source_id
                    .as_ref()
                    .map(|source_id| (AuthoredPlacementKey::from_edit(edit), source_id.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if edits.is_empty() && deletes.is_empty() {
            return Ok(0);
        }

        let project_root = project_root
            .map(Path::to_path_buf)
            .ok_or_else(|| "in-game editor save requires a project root".to_owned())?;

        let edits_by_key = edits
            .iter()
            .cloned()
            .map(|edit| (AuthoredPlacementKey::from_edit(&edit), edit))
            .collect::<BTreeMap<_, _>>();
        let mut map_refs = edits
            .iter()
            .map(|edit| edit.map_ref.clone())
            .collect::<BTreeSet<_>>();
        map_refs.extend(creates.keys().map(|key| key.map_ref.clone()));
        map_refs.extend(deletes.iter().map(|key| key.map_ref.clone()));

        let mut total = 0usize;
        let mut saved_entities = Vec::new();
        for map_ref in map_refs {
            let source_path = authored_source_path(&project_root, &map_ref)?;
            let original = std::fs::read_to_string(&source_path).map_err(|error| {
                format!(
                    "in-game editor save cannot read authored YMAP source '{}': {error}",
                    source_path.display()
                )
            })?;
            let mut updated = original.clone();
            let mut map_count = 0usize;

            // Create before delete so "duplicate then delete original" remains valid:
            // the new element can still clone the original source tag in this transaction.
            for (target, source_placement_id) in
                creates.iter().filter(|(key, _)| key.map_ref == map_ref)
            {
                let edit = edits_by_key.get(target).ok_or_else(|| {
                    format!(
                        "pending authored duplicate map='{}' placement='{}' has no live primary actor",
                        target.map_ref, target.placement_id
                    )
                })?;
                clone_tag_by_id(
                    &mut updated,
                    authored_tag(target.source()),
                    source_placement_id,
                    &target.placement_id,
                )
                .map_err(|error| {
                    format!(
                        "project save cannot duplicate {} id='{}' as id='{}' map='{}': {error}",
                        authored_tag(target.source()),
                        source_placement_id,
                        target.placement_id,
                        target.map_ref
                    )
                })?;
                patch_authored_transform(&mut updated, edit)?;
                map_count = map_count.saturating_add(1);
            }

            for edit in edits.iter().filter(|edit| edit.map_ref == map_ref) {
                let key = AuthoredPlacementKey::from_edit(edit);
                if creates.contains_key(&key) || deletes.contains(&key) {
                    continue;
                }
                patch_authored_transform(&mut updated, edit)?;
                map_count = map_count.saturating_add(1);
            }

            for key in deletes.iter().filter(|key| key.map_ref == map_ref) {
                remove_tag_by_id(&mut updated, authored_tag(key.source()), &key.placement_id)
                    .map_err(|error| {
                        format!(
                            "project save cannot delete {} id='{}' map='{}': {error}",
                            authored_tag(key.source()),
                            key.placement_id,
                            key.map_ref
                        )
                    })?;
                map_count = map_count.saturating_add(1);
            }

            if updated != original {
                std::fs::write(&source_path, updated.as_bytes()).map_err(|error| {
                    format!(
                        "in-game editor save cannot write authored YMAP source '{}': {error}",
                        source_path.display()
                    )
                })?;

                if let Err(error) = rebuild_compiled_ymap(&map_ref, &updated) {
                    let rollback = std::fs::write(&source_path, original.as_bytes());
                    return Err(match rollback {
                        Ok(()) => format!(
                            "in-game editor project save rolled back source after YMAP rebuild failed map='{map_ref}': {error}"
                        ),
                        Err(rollback_error) => format!(
                            "in-game editor YMAP rebuild failed map='{map_ref}': {error}; source rollback also failed path='{}' err='{rollback_error}'",
                            source_path.display()
                        ),
                    });
                }
            }

            total = total.saturating_add(map_count);
            saved_entities.extend(
                edits
                    .iter()
                    .filter(|edit| edit.map_ref == map_ref)
                    .map(|edit| edit.entity),
            );
            newengine_ulog_api::ulog::info!(
                "in-game editor project save: map='{}' source='{}' changes={} output='compiled NEF8 YMAP + VFS reload'",
                map_ref,
                source_path.display(),
                map_count,
            );
        }

        if !saved_entities.is_empty() {
            for entity in saved_entities {
                if world.exists(entity) {
                    let _ = world.remove::<AuthoredMapPlacementDirty>(entity);
                    let _ = world.remove::<AuthoredMapPlacementCloneSource>(entity);
                }
            }
        }

        let mut journal = self.journal.lock();
        journal.deletes.retain(|key| !deletes.contains(key));
        Ok(total)
    }

    fn collect_authored_placement_edits(
        &self,
        world: &World,
    ) -> Result<Vec<AuthoredPlacementEdit>, String> {
        let mut keys = BTreeSet::<AuthoredPlacementKey>::new();
        let mut edits = Vec::new();
        for (entity, authored) in world.query::<AuthoredMapPlacement>() {
            if !authored.primary || world.get::<AuthoredMapPlacementDirty>(entity).is_none() {
                continue;
            }
            let Some(transform) = world.get::<Transform>(entity).copied() else {
                continue;
            };
            let key = AuthoredPlacementKey::from_authored(authored);
            if !keys.insert(key) {
                return Err(format!(
                    "in-game editor save found duplicate authored identity map='{}' placement='{}'",
                    authored.map_ref, authored.placement_id
                ));
            }
            edits.push(AuthoredPlacementEdit {
                entity,
                map_ref: authored.map_ref.clone(),
                placement_id: authored.placement_id.clone(),
                source: authored.source,
                transform,
                clone_source_id: world
                    .get::<AuthoredMapPlacementCloneSource>(entity)
                    .map(|origin| origin.placement_id.clone()),
            });
        }
        Ok(edits)
    }

    pub fn authored_project_edit_status(&self, world: &World) -> AuthoredProjectEditStatus {
        // Snapshot journal state before taking the scene read lock. Duplicate/delete
        // paths inspect the World first and then update the journal; releasing this
        // lock preserves one lock order and avoids an editor-frame deadlock.
        let (mut dirty, pending_deletes, last_save_succeeded, last_save_message) = {
            let journal = self.journal.lock();
            (
                journal.deletes.clone(),
                journal.deletes.len(),
                journal.last_save_succeeded,
                journal.last_save_message.clone(),
            )
        };
        let mut pending_creates = 0usize;
        {
            for (entity, authored) in world.query::<AuthoredMapPlacement>() {
                if !authored.primary {
                    continue;
                }
                if world
                    .get::<AuthoredMapPlacementCloneSource>(entity)
                    .is_some()
                {
                    pending_creates = pending_creates.saturating_add(1);
                }
                if world.get::<AuthoredMapPlacementDirty>(entity).is_some() {
                    dirty.insert(AuthoredPlacementKey::from_authored(authored));
                }
            }
        }
        AuthoredProjectEditStatus {
            dirty_placements: dirty.len(),
            pending_creates,
            pending_deletes,
            last_save_succeeded,
            last_save_message,
        }
    }

    pub fn prepare_authored_duplicate(
        &self,
        world: &World,
        source_entity: EntityId,
        source: &AuthoredMapPlacement,
    ) -> Option<(AuthoredMapPlacement, AuthoredMapPlacementCloneSource)> {
        if !source.primary
            || source.map_ref.trim().is_empty()
            || source.placement_id.trim().is_empty()
        {
            return None;
        }

        let template_id = world
            .get::<AuthoredMapPlacementCloneSource>(source_entity)
            .map(|origin| origin.placement_id.clone())
            .unwrap_or_else(|| source.placement_id.clone());
        let mut journal = self.journal.lock();

        loop {
            journal.next_copy_sequence = journal.next_copy_sequence.saturating_add(1);
            let candidate = format!(
                "{}_copy_{:03}",
                source.placement_id, journal.next_copy_sequence
            );
            let candidate_key =
                AuthoredPlacementKey::new(source.map_ref.clone(), candidate.clone(), source.source);
            let occupied_in_world = world.query::<AuthoredMapPlacement>().any(|(_, authored)| {
                AuthoredPlacementKey::from_authored(authored) == candidate_key
            });
            if occupied_in_world || journal.deletes.contains(&candidate_key) {
                continue;
            }

            journal.last_save_succeeded = None;
            journal.last_save_message = "Unsaved authored map changes".to_owned();
            return Some((
                AuthoredMapPlacement::new(source.map_ref.clone(), candidate, source.source, true),
                AuthoredMapPlacementCloneSource::new(template_id),
            ));
        }
    }

    pub fn record_authored_deletion(&self, authored: &AuthoredMapPlacement) {
        if !authored.primary {
            return;
        }
        let key = AuthoredPlacementKey::from_authored(authored);
        let mut journal = self.journal.lock();
        journal.deletes.insert(key);
        journal.last_save_succeeded = None;
        journal.last_save_message = "Unsaved authored map changes".to_owned();
    }
}

impl SceneAuthoringService for SceneAuthoringRuntime {
    fn in_game_editor_enabled(&self) -> bool {
        SceneAuthoringRuntime::in_game_editor_enabled(self)
    }

    fn set_in_game_editor_enabled(&self, enabled: bool) -> bool {
        SceneAuthoringRuntime::set_in_game_editor_enabled(self, enabled)
    }

    fn save_authored_project_world(
        &self,
        world: &mut World,
        project_root: Option<&Path>,
    ) -> Result<usize, String> {
        SceneAuthoringRuntime::save_authored_project_world(self, world, project_root)
    }

    fn authored_project_edit_status(&self, world: &World) -> AuthoredProjectEditStatus {
        SceneAuthoringRuntime::authored_project_edit_status(self, world)
    }

    fn prepare_authored_duplicate(
        &self,
        world: &World,
        source_entity: EntityId,
        source: &AuthoredMapPlacement,
    ) -> Option<(AuthoredMapPlacement, AuthoredMapPlacementCloneSource)> {
        SceneAuthoringRuntime::prepare_authored_duplicate(self, world, source_entity, source)
    }

    fn record_authored_deletion(&self, authored: &AuthoredMapPlacement) {
        SceneAuthoringRuntime::record_authored_deletion(self, authored)
    }
}

#[inline]
const fn authored_source_key(source: AuthoredMapPlacementSource) -> u8 {
    match source {
        AuthoredMapPlacementSource::ProfilePrefab => 1,
        AuthoredMapPlacementSource::DiscretePlacement => 2,
    }
}

#[inline]
const fn authored_source_from_key(key: u8) -> AuthoredMapPlacementSource {
    match key {
        2 => AuthoredMapPlacementSource::DiscretePlacement,
        _ => AuthoredMapPlacementSource::ProfilePrefab,
    }
}

#[inline]
const fn authored_tag(source: AuthoredMapPlacementSource) -> &'static str {
    match source {
        AuthoredMapPlacementSource::ProfilePrefab => "Prefab",
        AuthoredMapPlacementSource::DiscretePlacement => "Placement",
    }
}

fn authored_source_path(project_root: &Path, map_ref: &str) -> Result<PathBuf, String> {
    let logical = map_ref
        .split('@')
        .next()
        .unwrap_or(map_ref)
        .trim()
        .replace('\\', "/");
    if logical.is_empty()
        || logical.starts_with('/')
        || logical.contains("../")
        || logical.contains(":/")
    {
        return Err(format!(
            "unsafe authored map ref for project save: '{map_ref}'"
        ));
    }
    if !logical.to_ascii_lowercase().ends_with(".ymap") {
        return Err(format!(
            "project save currently requires a .ymap authored source, got '{map_ref}'"
        ));
    }
    Ok(
        newengine_project_api::ProjectFilesystem::new(project_root.to_path_buf())
            .source_dir()
            .join(format!("{logical}.xml")),
    )
}

fn patch_authored_transform(xml: &mut String, edit: &AuthoredPlacementEdit) -> Result<(), String> {
    let (yaw, pitch, roll) = edit.transform.rotation.to_euler(EulerRot::YXZ);
    let position = format_vec3(edit.transform.position);
    let rotation = format_triplet([yaw, pitch, roll]);
    let scale = format_vec3(edit.transform.scale);
    patch_tag_attributes_by_id(
        xml,
        authored_tag(edit.source),
        &edit.placement_id,
        &[
            ("position", position.as_str()),
            ("rotation_ypr", rotation.as_str()),
            ("scale", scale.as_str()),
        ],
    )
    .map_err(|error| {
        format!(
            "project save cannot patch {} id='{}' map='{}': {error}",
            authored_tag(edit.source),
            edit.placement_id,
            edit.map_ref
        )
    })
}

fn format_vec3(value: Vec3) -> String {
    format_triplet([value.x, value.y, value.z])
}

fn format_triplet(value: [f32; 3]) -> String {
    format!(
        "{},{},{}",
        format_scalar(value[0]),
        format_scalar(value[1]),
        format_scalar(value[2])
    )
}

fn format_scalar(value: f32) -> String {
    let value = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.push('0');
    }
    text
}

fn find_tag_span_by_id(
    xml: &str,
    tag: &str,
    id: &str,
) -> Result<Option<(usize, usize, usize)>, String> {
    let needle = format!("<{tag}");
    let mut search_from = 0usize;
    while let Some(relative) = xml[search_from..].find(&needle) {
        let start = search_from + relative;
        let opening_end = xml[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| format!("unterminated <{tag}> tag"))?;
        let opening = &xml[start..opening_end];
        if xml_attribute_value(opening, "id").as_deref() == Some(id) {
            let element_end = if opening.trim_end().ends_with("/>") {
                opening_end
            } else {
                let closing = format!("</{tag}>");
                xml[opening_end..]
                    .find(&closing)
                    .map(|offset| opening_end + offset + closing.len())
                    .ok_or_else(|| format!("unterminated <{tag}> element id='{id}'"))?
            };
            return Ok(Some((start, opening_end, element_end)));
        }
        search_from = opening_end;
    }
    Ok(None)
}

fn patch_tag_attributes_by_id(
    xml: &mut String,
    tag: &str,
    id: &str,
    attributes: &[(&str, &str)],
) -> Result<(), String> {
    let Some((start, opening_end, _)) = find_tag_span_by_id(xml, tag, id)? else {
        return Err(format!("<{tag}> with id='{id}' was not found"));
    };
    let mut patched = xml[start..opening_end].to_owned();
    for (name, value) in attributes {
        patched = set_xml_attribute(&patched, name, value);
    }
    xml.replace_range(start..opening_end, &patched);
    Ok(())
}

fn clone_tag_by_id(
    xml: &mut String,
    tag: &str,
    source_id: &str,
    target_id: &str,
) -> Result<bool, String> {
    if find_tag_span_by_id(xml, tag, target_id)?.is_some() {
        return Ok(false);
    }
    let Some((source_start, opening_end, source_end)) = find_tag_span_by_id(xml, tag, source_id)?
    else {
        return Err(format!("<{tag}> with id='{source_id}' was not found"));
    };

    let source_element = xml[source_start..source_end].to_owned();
    let opening_len = opening_end - source_start;
    let mut target_opening = source_element[..opening_len].to_owned();
    target_opening = set_xml_attribute(&target_opening, "id", target_id);
    let mut target_element = source_element;
    target_element.replace_range(..opening_len, &target_opening);

    let line_start = xml[..source_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let indent = &xml[line_start..source_start];
    let indent = if indent.chars().all(char::is_whitespace) {
        indent
    } else {
        ""
    };
    xml.insert_str(source_end, &format!("\n{indent}{target_element}"));
    Ok(true)
}

fn remove_tag_by_id(xml: &mut String, tag: &str, id: &str) -> Result<bool, String> {
    let Some((start, _, element_end)) = find_tag_span_by_id(xml, tag, id)? else {
        return Ok(false);
    };
    let line_start = xml[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(start);
    let remove_start = if xml[line_start..start].chars().all(char::is_whitespace) {
        line_start
    } else {
        start
    };
    let mut remove_end = element_end;
    if xml[remove_end..].starts_with("\r\n") {
        remove_end += 2;
    } else if xml[remove_end..].starts_with('\n') {
        remove_end += 1;
    }
    xml.replace_range(remove_start..remove_end, "");
    Ok(true)
}

fn xml_attribute_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let rel = tag[cursor..].find(name)?;
        let start = cursor + rel;
        let before_ok = start == 0 || !is_xml_name_byte(bytes[start - 1]);
        let after = start + name.len();
        let after_ok = after >= bytes.len() || !is_xml_name_byte(bytes[after]);
        if !before_ok || !after_ok {
            cursor = after;
            continue;
        }
        let mut i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            cursor = after;
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = *bytes.get(i)?;
        if quote != b'"' && quote != b'\'' {
            cursor = after;
            continue;
        }
        let value_start = i + 1;
        let value_end = tag[value_start..].find(quote as char)? + value_start;
        return Some(tag[value_start..value_end].to_owned());
    }
    None
}

fn set_xml_attribute(tag: &str, name: &str, value: &str) -> String {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let Some(rel) = tag[cursor..].find(name) else {
            break;
        };
        let start = cursor + rel;
        let before_ok = start == 0 || !is_xml_name_byte(bytes[start - 1]);
        let after = start + name.len();
        let after_ok = after >= bytes.len() || !is_xml_name_byte(bytes[after]);
        if !before_ok || !after_ok {
            cursor = after;
            continue;
        }
        let mut i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            cursor = after;
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let Some(&quote) = bytes.get(i) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            cursor = after;
            continue;
        }
        let value_start = i + 1;
        let Some(rel_end) = tag[value_start..].find(quote as char) else {
            break;
        };
        let value_end = value_start + rel_end;
        let mut out = tag.to_owned();
        out.replace_range(value_start..value_end, value);
        return out;
    }

    let insertion = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    let mut out = tag.to_owned();
    out.insert_str(insertion, &format!(" {name}=\"{value}\""));
    out
}

#[inline]
fn is_xml_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.')
}

fn rebuild_compiled_ymap(map_ref: &str, xml: &str) -> Result<(), String> {
    let logical_path = map_ref
        .split('@')
        .next()
        .unwrap_or(map_ref)
        .trim()
        .replace('\\', "/");
    let payload = serde_json::to_vec(&serde_json::json!({
        "logical_path": logical_path,
        "operation": "replace_body",
        "payload_text": xml,
        "verify_after_build": true,
        "dry_run": false,
    }))
    .map_err(|error| format!("encode YMAP package-writer request failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_PACKAGE_WRITER_SERVICE_ID,
        newengine_assets_api::method::LIST_FILE_REPACK_JSON_V1,
        &payload,
    )?
    .ok_or_else(|| {
        format!(
            "assets package-writer gateway '{}' is unavailable",
            newengine_assets_api::ENGINE_ASSETS_PACKAGE_WRITER_SERVICE_ID
        )
    })?;
    let response: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("package-writer returned invalid JSON: {error}"))?;
    if response.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
        || response.get("applied").and_then(serde_json::Value::as_bool) != Some(true)
    {
        return Err(response
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("YMAP package-writer rejected save")
            .to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_collection_contains_only_editor_dirty_authored_actors() {
        let runtime = SceneAuthoringRuntime::default();
        let mut world = World::new();
        let dirty = world.spawn();
        let _ = world.insert(dirty, Transform::default());
        let _ = world.insert(
            dirty,
            AuthoredMapPlacement::new(
                "maps/test.ymap",
                "dirty",
                AuthoredMapPlacementSource::ProfilePrefab,
                true,
            ),
        );
        let _ = world.insert(dirty, AuthoredMapPlacementDirty);
        let clean = world.spawn();
        let _ = world.insert(clean, Transform::default());
        let _ = world.insert(
            clean,
            AuthoredMapPlacement::new(
                "maps/test.ymap",
                "clean",
                AuthoredMapPlacementSource::ProfilePrefab,
                true,
            ),
        );
        let replica = world.spawn();
        let _ = world.insert(replica, Transform::default());
        let _ = world.insert(
            replica,
            AuthoredMapPlacement::new(
                "maps/test.ymap",
                "dirty",
                AuthoredMapPlacementSource::ProfilePrefab,
                false,
            ),
        );
        let _ = world.insert(replica, AuthoredMapPlacementDirty);
        let edits = runtime.collect_authored_placement_edits(&world).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].placement_id, "dirty");
    }

    #[test]
    fn duplicate_uses_original_source_and_allocates_unique_authored_id() {
        let runtime = SceneAuthoringRuntime::default();
        let mut world = World::new();
        let source_entity = world.spawn();
        let authored = AuthoredMapPlacement::new(
            "maps/test.ymap",
            "oak",
            AuthoredMapPlacementSource::ProfilePrefab,
            true,
        );
        let _ = world.insert(source_entity, authored.clone());
        let (duplicate, origin) = runtime
            .prepare_authored_duplicate(&world, source_entity, &authored)
            .unwrap();
        assert_eq!(duplicate.placement_id, "oak_copy_001");
        assert_eq!(origin.placement_id, "oak");
    }

    #[test]
    fn project_source_path_maps_content_ymap_to_source_xml() {
        let root = Path::new("C:/Project");
        let path = authored_source_path(root, "maps/world.ymap@map").unwrap();
        assert!(path.ends_with(Path::new("Source/maps/world.ymap.xml")));
    }

    #[test]
    fn patches_only_matching_prefab_transform() {
        let mut xml = r#"<YmapMapDefinition><map><profile><prefabs>
<Prefab id="alpha" position="1,2,3" rotation_ypr="0,0,0" scale="1,1,1" />
<Prefab id="beta" position="4,5,6" rotation_ypr="0,0,0" scale="1,1,1" />
</prefabs></profile></map></YmapMapDefinition>"#
            .to_owned();
        patch_tag_attributes_by_id(
            &mut xml,
            "Prefab",
            "beta",
            &[
                ("position", "10,20,30"),
                ("rotation_ypr", "0.1,0.2,0.3"),
                ("scale", "2,2,2"),
            ],
        )
        .unwrap();
        assert!(xml.contains("id=\"alpha\" position=\"1,2,3\""));
        assert!(xml.contains(
            "id=\"beta\" position=\"10,20,30\" rotation_ypr=\"0.1,0.2,0.3\" scale=\"2,2,2\""
        ));
    }

    #[test]
    fn duplicate_clones_complete_prefab_tag_and_delete_removes_only_target() {
        let mut xml = r#"<prefabs>
  <Prefab id="oak" source="models/oak.ydd@oak" material="materials/oak.nemat@bark" position="0,0,0" />
  <Prefab id="rock" source="models/rock.ydd@rock" position="1,0,0" />
</prefabs>"#.to_owned();
        assert!(clone_tag_by_id(&mut xml, "Prefab", "oak", "oak_copy_001").unwrap());
        assert!(xml.contains("id=\"oak_copy_001\" source=\"models/oak.ydd@oak\" material=\"materials/oak.nemat@bark\""));
        assert!(remove_tag_by_id(&mut xml, "Prefab", "oak_copy_001").unwrap());
        assert!(!xml.contains("oak_copy_001"));
        assert!(xml.contains("id=\"oak\""));
        assert!(xml.contains("id=\"rock\""));
    }
}
