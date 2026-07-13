use super::*;

pub(crate) fn parse_binding_plan(xml: &str, document_ref: &str, surface_id: &str) -> UiBindingPlan {
    let mut plan = UiBindingPlan {
        document_ref: document_ref.to_owned(),
        surface_id: surface_id.to_owned(),
        ..Default::default()
    };
    if let Some(graph) = first_element(xml, "BindingGraph") {
        for source in elements(&graph.inner, "StateSource") {
            plan.state_sources.push(UiStateSource {
                id: attr_value(&source.open, "id").unwrap_or_default(),
                source: attr_value(&source.open, "source").unwrap_or_default(),
                contract: attr_value(&source.open, "contract").unwrap_or_default(),
                update_policy: update_policy_from_attr(
                    attr_value(&source.open, "update").as_deref(),
                ),
            });
        }
        for bind in elements(&graph.inner, "Bind") {
            plan.bindings.push(UiBindingEdge {
                element_id: attr_value(&bind.open, "element").unwrap_or_default(),
                property: attr_value(&bind.open, "property").unwrap_or_default(),
                source_id: attr_value(&bind.open, "source_id").unwrap_or_default(),
                path: attr_value(&bind.open, "source").unwrap_or_default(),
                mode: UiBindingMode::OneWay,
                fallback: attr_value(&bind.open, "fallback"),
                transform: attr_value(&bind.open, "transform"),
            });
        }
    }
    for action in elements(xml, "Action") {
        if let Some(action_id) = attr_value(&action.open, "id") {
            plan.actions.push(UiActionEdge {
                element_id: attr_value(&action.open, "element").unwrap_or_default(),
                trigger: attr_value(&action.open, "trigger").unwrap_or_else(|| "click".to_owned()),
                action_id,
                target_gateway: attr_value(&action.open, "target").unwrap_or_default(),
                command: attr_value(&action.open, "command")
                    .or_else(|| attr_value(&action.open, "event"))
                    .unwrap_or_default(),
                payload_schema: None,
            });
        }
    }
    plan
}

pub(crate) fn update_policy_from_attr(value: Option<&str>) -> UiUpdatePolicy {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "frame" => UiUpdatePolicy::Frame,
        "event" => UiUpdatePolicy::Event,
        "dirty" => UiUpdatePolicy::Dirty,
        "manual" => UiUpdatePolicy::Manual,
        _ => UiUpdatePolicy::OnChange,
    }
}

pub(crate) fn extend_binding_plan_with_inline_nodes(
    plan: &mut UiBindingPlan,
    root: &UiNodeRequest,
) {
    append_node_bindings(plan, root);
    plan.bindings.sort_by(|left, right| {
        left.element_id
            .cmp(&right.element_id)
            .then_with(|| left.property.cmp(&right.property))
            .then_with(|| left.source_id.cmp(&right.source_id))
            .then_with(|| left.path.cmp(&right.path))
    });
    plan.bindings.dedup_by(|left, right| {
        left.element_id == right.element_id
            && left.property == right.property
            && left.source_id == right.source_id
            && left.path == right.path
    });
}

fn append_node_bindings(plan: &mut UiBindingPlan, node: &UiNodeRequest) {
    for binding in &node.bindings {
        if binding.source.trim().is_empty()
            || binding.path.trim().is_empty()
            || binding.property.trim().is_empty()
        {
            continue;
        }
        plan.bindings.push(UiBindingEdge {
            element_id: node.id.clone(),
            property: binding.property.clone(),
            source_id: binding.source.clone(),
            path: binding.path.clone(),
            mode: binding_mode_from_text(binding.mode.as_str()),
            fallback: binding_fallback_text(&binding.fallback),
            transform: None,
        });
    }
    for child in &node.children {
        append_node_bindings(plan, child);
    }
}

fn binding_mode_from_text(value: &str) -> UiBindingMode {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_'], "")
        .as_str()
    {
        "twoway" | "readwrite" => UiBindingMode::TwoWay,
        "event" => UiBindingMode::Event,
        _ => UiBindingMode::OneWay,
    }
}

fn binding_fallback_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod inline_binding_tests {
    use super::*;

    #[test]
    fn inline_bindings_are_projected_into_runtime_binding_plan() {
        let mut root = UiNodeRequest::new("root", UiRuntimeNodeKind::Panel);
        let mut child = UiNodeRequest::new("entry.name", UiRuntimeNodeKind::Text);
        child.bindings.push(UiNodeBindingRequest {
            property: "text".to_owned(),
            source: "entry_00".to_owned(),
            path: "name".to_owned(),
            mode: "read".to_owned(),
            fallback: serde_json::json!("fallback"),
        });
        root.children.push(child);

        let mut plan = UiBindingPlan::default();
        extend_binding_plan_with_inline_nodes(&mut plan, &root);

        assert_eq!(plan.bindings.len(), 1);
        let binding = &plan.bindings[0];
        assert_eq!(binding.element_id, "entry.name");
        assert_eq!(binding.property, "text");
        assert_eq!(binding.source_id, "entry_00");
        assert_eq!(binding.path, "name");
        assert_eq!(binding.fallback.as_deref(), Some("fallback"));
    }

    #[test]
    fn explicit_and_inline_duplicate_edges_are_deduplicated() {
        let mut root = UiNodeRequest::new("root", UiRuntimeNodeKind::Panel);
        let mut child = UiNodeRequest::new("title", UiRuntimeNodeKind::Text);
        child.bindings.push(UiNodeBindingRequest {
            property: "text".to_owned(),
            source: "shell".to_owned(),
            path: "title".to_owned(),
            ..UiNodeBindingRequest::default()
        });
        root.children.push(child);
        let edge = UiBindingEdge {
            element_id: "title".to_owned(),
            property: "text".to_owned(),
            source_id: "shell".to_owned(),
            path: "title".to_owned(),
            ..UiBindingEdge::default()
        };
        let mut plan = UiBindingPlan {
            bindings: vec![edge],
            ..UiBindingPlan::default()
        };

        extend_binding_plan_with_inline_nodes(&mut plan, &root);
        assert_eq!(plan.bindings.len(), 1);
    }
}
