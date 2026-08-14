use super::*;
use newengine_runtime_session_api::{
    RuntimeSessionControlMode, RuntimeSessionId, RuntimeSessionState, RuntimeWorldChangeKind,
    RuntimeWorldChangeV1, RuntimeWorldDiffV1,
};
use newengine_runtime_session_runtime::acknowledge_apply_changes_request;
use newengine_world_api::{WorldClient, WorldLoadSnapshotRequest, WorldSnapshotResponse};

const PIE_DIFF_MAX_CHANGES: usize = 512;
const PIE_DIFF_PERIOD_FRAMES: u64 = 15;

#[derive(Clone, Debug, Default)]
pub(super) struct EditorPieWorldState {
    session_id: RuntimeSessionId,
    baseline: Option<WorldSnapshotResponse>,
    pending_apply: Option<PendingPieApply>,
    last_diff_frame: u64,
}

#[derive(Clone, Debug)]
struct PendingPieApply {
    apply_after_frame: u64,
    runtime_snapshot: WorldSnapshotResponse,
}

pub(super) fn install_editor_pie_world_state(resources: &mut Resources) {
    if resources.get::<EditorPieWorldState>().is_none() {
        resources.insert(EditorPieWorldState::default());
    }
    if resources.get::<RuntimeWorldDiffV1>().is_none() {
        resources.insert(RuntimeWorldDiffV1::default());
    }
}

/// Capture the live PIE scene before the session command changes mode to Edit.
/// The actual authored scene payload is applied on the following frame, after
/// engine-runtime has restored its gameplay/ECS Play snapshot.
pub(super) fn stage_apply_changes_from_pie(
    resources: &mut Resources,
    frame_index: u64,
) -> Result<(), String> {
    install_editor_pie_world_state(resources);
    let mut state = resources
        .remove::<EditorPieWorldState>()
        .unwrap_or_default();
    let result = (|| {
        if state.baseline.is_none() {
            return Err(
                "PIE baseline world snapshot is unavailable; refusing destructive apply".to_owned(),
            );
        }
        let world = WorldClient::new(newengine_plugin_host::default_host_api());
        let runtime_snapshot = world
            .snapshot_response_json_v1()
            .map_err(|error| format!("capture live PIE world before Apply Changes: {error}"))?;
        if runtime_snapshot.scene_payload.is_none() {
            return Err("live PIE world snapshot has no authored scene payload".to_owned());
        }
        state.pending_apply = Some(PendingPieApply {
            apply_after_frame: frame_index.saturating_add(1),
            runtime_snapshot,
        });
        Ok(())
    })();
    resources.insert(state);
    result
}

pub(super) fn sync_editor_pie_world_state(
    resources: &mut Resources,
    frame_index: u64,
    session: &RuntimeSessionState,
) {
    install_editor_pie_world_state(resources);
    let mut state = resources
        .remove::<EditorPieWorldState>()
        .unwrap_or_default();
    let world = WorldClient::new(newengine_plugin_host::default_host_api());

    if session.is_active() && state.session_id != session.session_id {
        state.session_id = session.session_id;
        state.pending_apply = None;
        state.last_diff_frame = 0;
        match world.snapshot_response_json_v1() {
            Ok(snapshot) => {
                state.baseline = Some(snapshot);
                resources.insert(RuntimeWorldDiffV1::empty(
                    session.session_id,
                    frame_index,
                    "PIE baseline captured",
                ));
            }
            Err(error) => {
                state.baseline = None;
                resources.insert(RuntimeWorldDiffV1::empty(
                    session.session_id,
                    frame_index,
                    format!("PIE baseline unavailable: {error}"),
                ));
                newengine_ulog_api::ulog::warn!("PIE world baseline capture failed: {}", error);
            }
        }
    }

    if session.is_active() && state.baseline.is_some() {
        let should_refresh = state.last_diff_frame == 0
            || frame_index.saturating_sub(state.last_diff_frame) >= PIE_DIFF_PERIOD_FRAMES
            || session.paused
            || session.control_mode == RuntimeSessionControlMode::Ejected;
        if should_refresh {
            state.last_diff_frame = frame_index;
            if let Ok(current) = world.snapshot_response_json_v1() {
                if let Some(baseline) = state.baseline.as_ref() {
                    resources.insert(diff_world_snapshots(
                        session.session_id,
                        frame_index,
                        baseline,
                        &current,
                    ));
                }
            }
        }
    }

    if !session.is_active() {
        if let Some(pending) = state.pending_apply.clone() {
            if frame_index >= pending.apply_after_frame {
                let result =
                    apply_pending_scene(&world, state.baseline.as_ref(), &pending.runtime_snapshot);
                match result {
                    Ok(()) => {
                        newengine_ulog_api::ulog::info!(
                            "PIE Apply Changes: authored scene payload applied after runtime snapshot restore"
                        );
                    }
                    Err(error) => {
                        newengine_ulog_api::ulog::warn!("PIE Apply Changes failed: {}", error);
                    }
                }
                state.pending_apply = None;
                state.baseline = None;
                acknowledge_apply_changes_request(resources);
            }
        } else if !session.apply_changes_requested {
            state.baseline = None;
        }
    }

    resources.insert(state);
}

fn apply_pending_scene(
    world: &WorldClient,
    baseline: Option<&WorldSnapshotResponse>,
    runtime: &WorldSnapshotResponse,
) -> Result<(), String> {
    let baseline = baseline.ok_or_else(|| "PIE baseline vanished before apply".to_owned())?;
    let scene_payload = runtime
        .scene_payload
        .clone()
        .ok_or_else(|| "runtime PIE snapshot has no scene payload".to_owned())?;
    let merged = WorldSnapshotResponse {
        schema: baseline.schema.clone(),
        state: baseline.state.clone(),
        scene_payload: Some(scene_payload),
    };
    let response = world.load_snapshot_json_v1(&WorldLoadSnapshotRequest {
        snapshot: Some(merged),
        payload: None,
        replace_scene: true,
    })?;
    if !response.ok {
        return Err("engine.world rejected PIE authored-scene apply".to_owned());
    }
    Ok(())
}

fn diff_world_snapshots(
    session_id: RuntimeSessionId,
    frame_index: u64,
    baseline: &WorldSnapshotResponse,
    current: &WorldSnapshotResponse,
) -> RuntimeWorldDiffV1 {
    let Some(before) = baseline.scene_payload.as_ref() else {
        return RuntimeWorldDiffV1::empty(session_id, frame_index, "baseline has no scene payload");
    };
    let Some(after) = current.scene_payload.as_ref() else {
        return RuntimeWorldDiffV1::empty(
            session_id,
            frame_index,
            "current world has no scene payload",
        );
    };
    let mut changes = Vec::new();
    let mut truncated = false;
    diff_json_value("", before, after, &mut changes, &mut truncated);
    RuntimeWorldDiffV1 {
        version: 1,
        session_id,
        frame_index,
        change_count: changes.len(),
        changes,
        truncated,
        reason: "authored scene diff between editor baseline and live PIE world".to_owned(),
    }
}

fn diff_json_value(
    path: &str,
    before: &serde_json::Value,
    after: &serde_json::Value,
    out: &mut Vec<RuntimeWorldChangeV1>,
    truncated: &mut bool,
) {
    if before == after || *truncated {
        return;
    }
    if out.len() >= PIE_DIFF_MAX_CHANGES {
        *truncated = true;
        return;
    }
    match (before, after) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            let keys = a.keys().chain(b.keys()).cloned().collect::<BTreeSet<_>>();
            for key in keys {
                let child = format!("{}/{}", path, escape_json_pointer(&key));
                match (a.get(&key), b.get(&key)) {
                    (Some(x), Some(y)) => diff_json_value(&child, x, y, out, truncated),
                    (Some(x), None) => push_change(
                        out,
                        truncated,
                        child,
                        RuntimeWorldChangeKind::Removed,
                        Some(x),
                        None,
                    ),
                    (None, Some(y)) => push_change(
                        out,
                        truncated,
                        child,
                        RuntimeWorldChangeKind::Added,
                        None,
                        Some(y),
                    ),
                    (None, None) => {}
                }
                if *truncated {
                    break;
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            let len = a.len().max(b.len());
            for index in 0..len {
                let child = format!("{}/{}", path, index);
                match (a.get(index), b.get(index)) {
                    (Some(x), Some(y)) => diff_json_value(&child, x, y, out, truncated),
                    (Some(x), None) => push_change(
                        out,
                        truncated,
                        child,
                        RuntimeWorldChangeKind::Removed,
                        Some(x),
                        None,
                    ),
                    (None, Some(y)) => push_change(
                        out,
                        truncated,
                        child,
                        RuntimeWorldChangeKind::Added,
                        None,
                        Some(y),
                    ),
                    (None, None) => {}
                }
                if *truncated {
                    break;
                }
            }
        }
        _ => push_change(
            out,
            truncated,
            if path.is_empty() {
                "/".to_owned()
            } else {
                path.to_owned()
            },
            RuntimeWorldChangeKind::Modified,
            Some(before),
            Some(after),
        ),
    }
}

fn push_change(
    out: &mut Vec<RuntimeWorldChangeV1>,
    truncated: &mut bool,
    path: String,
    kind: RuntimeWorldChangeKind,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) {
    if out.len() >= PIE_DIFF_MAX_CHANGES {
        *truncated = true;
        return;
    }
    out.push(RuntimeWorldChangeV1 {
        path,
        kind,
        before_json: before.map(compact_json),
        after_json: after.map(compact_json),
    });
}

fn compact_json(value: &serde_json::Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned());
    if text.len() <= 512 {
        text
    } else {
        format!("{}…", &text[..512])
    }
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_reports_added_removed_and_modified_paths() {
        let before = serde_json::json!({"a":1,"b":{"x":2},"gone":true});
        let after = serde_json::json!({"a":3,"b":{"x":2,"y":4},"new":false});
        let mut out = Vec::new();
        let mut truncated = false;
        diff_json_value("", &before, &after, &mut out, &mut truncated);
        let paths = out
            .iter()
            .map(|change| change.path.as_str())
            .collect::<BTreeSet<_>>();
        assert!(paths.contains("/a"));
        assert!(paths.contains("/b/y"));
        assert!(paths.contains("/gone"));
        assert!(paths.contains("/new"));
        assert!(!truncated);
    }
}
