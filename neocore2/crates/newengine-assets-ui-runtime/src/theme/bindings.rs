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
