use super::graph_ops::{push_edge, push_node};
use super::*;

pub struct AssetGraphResolver;

impl AssetGraphResolver {
    /// Classification-only resolver kept for dry-run tests and callers that cannot
    /// access `engine.assets`. Runtime `engine.assets.graph` hydrates this graph by
    /// calling semantic gateways and attaching VFS/hash diagnostics.
    pub fn resolve_root_ref(root_ref: &str) -> ResolvedAssetGraphV2 {
        let root_ref = normalize_asset_ref(root_ref);
        let mut graph = ResolvedAssetGraphV2 {
            root_ref: root_ref.clone(),
            source: root_ref.clone(),
            cache_key_parts: cache_key_parts_for_ref(&root_ref),
            ..Default::default()
        };
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: begin root_ref='{root_ref}' mode='classification-only'"
        ));
        let (role, kind, gateway, method) = classify_ref(&root_ref);
        push_node(&mut graph, &root_ref, role, kind, gateway, method);
        finalize_graph(&mut graph);
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: root classified role='{role}' semantic_gateway='{gateway}'"
        ));
        graph
    }

    pub fn resolve_construction_plan(plan: &DataDrivenConstructionPlan) -> ResolvedAssetGraphV2 {
        let root = plan.source.trim();
        let mut graph = ResolvedAssetGraphV2 {
            root_ref: root.to_owned(),
            source: root.to_owned(),
            cache_key_parts: cache_key_parts_for_ref(root),
            ..Default::default()
        };
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: begin construction_plan source='{root}' objects={}",
            plan.objects.len()
        ));
        for object in &plan.objects {
            let definition_ref = normalize_asset_ref(&object.definition.logical_path);
            push_node(
                &mut graph,
                &definition_ref,
                ROLE_DEFINITION_ENTRIES,
                &object.definition.asset_kind,
                newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            );
            if let Some(drawable) = object.drawable.as_ref() {
                let drawable_ref = normalize_asset_ref(&drawable.logical_path);
                push_node(
                    &mut graph,
                    &drawable_ref,
                    ROLE_DRAWABLE_DICTIONARY,
                    &drawable.asset_kind,
                    "engine.model",
                    "model.drawable_dictionary_manifest_json_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &drawable_ref,
                    ROLE_DRAWABLE_DICTIONARY,
                    drawable.required,
                );
            } else {
                graph
                    .missing_refs
                    .push(format!("{}: missing drawable dictionary", object.name));
            }
            if let Some(texture_dictionary) = object.texture_dictionary.as_ref() {
                let texture_ref = normalize_asset_ref(&texture_dictionary.logical_path);
                push_node(
                    &mut graph,
                    &texture_ref,
                    ROLE_TEXTURE_DICTIONARY,
                    &texture_dictionary.asset_kind,
                    "engine.assets",
                    "asset.texture_dictionary_runtime_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &texture_ref,
                    ROLE_TEXTURE_DICTIONARY,
                    texture_dictionary.required,
                );
            } else {
                graph
                    .missing_refs
                    .push(format!("{}: missing texture dictionary", object.name));
            }
            if let Some(physics) = object.physics_dictionary.as_ref() {
                let physics_ref = normalize_asset_ref(&physics.logical_path);
                push_node(
                    &mut graph,
                    &physics_ref,
                    "physics_dictionary",
                    &physics.asset_kind,
                    "engine.physics",
                    "physics.frame_json_v1",
                );
                push_edge(
                    &mut graph,
                    &definition_ref,
                    &physics_ref,
                    "physics_dictionary",
                    physics.required,
                );
            }
            for slot in &object.material_slots {
                if slot.material.trim().is_empty() {
                    graph.missing_refs.push(format!(
                        "{}: material slot '{}' has empty material ref",
                        object.name, slot.slot
                    ));
                    continue;
                }
                let material_ref = normalize_asset_ref(&slot.material);
                push_node(
                    &mut graph,
                    &material_ref,
                    ROLE_MATERIAL_LIBRARY,
                    MATERIAL_LIBRARY_ASSET_KIND,
                    "engine.materials",
                    "materials.load_descriptor_v1",
                );
                if let Some(drawable) = object.drawable.as_ref() {
                    push_edge(
                        &mut graph,
                        &drawable.logical_path,
                        &material_ref,
                        &format!("material_slot/{}", slot.slot),
                        true,
                    );
                } else {
                    push_edge(
                        &mut graph,
                        &definition_ref,
                        &material_ref,
                        &format!("material_slot/{}", slot.slot),
                        true,
                    );
                }
            }
            graph.debug_log.push(format!(
                "assets.graph.resolve_v1: object='{}' graph nodes={} edges={}",
                object.name,
                graph.nodes.len(),
                graph.edges.len()
            ));
        }
        for warning in &plan.warnings {
            if warning
                .to_ascii_lowercase()
                .contains("retired texture dictionary")
            {
                graph.migration_warnings.push(warning.clone());
            } else {
                graph.format_warnings.push(warning.clone());
            }
        }
        finalize_graph(&mut graph);
        graph
    }

    pub fn validate_graph(graph: ResolvedAssetGraphV2) -> AssetGraphValidationResult {
        let mut errors = Vec::new();
        let mut warnings = graph.format_warnings.clone();
        warnings.extend(graph.metadata_warnings.clone());
        warnings.extend(graph.migration_warnings.clone());
        if graph.root_ref.trim().is_empty() {
            errors.push("asset graph root_ref is empty".to_owned());
        }
        if graph.nodes.is_empty() {
            errors.push("asset graph contains no nodes".to_owned());
        }
        errors.extend(
            graph
                .missing_refs
                .iter()
                .map(|it| format!("missing ref: {it}")),
        );
        errors.extend(graph.cycle_errors.iter().map(|it| format!("cycle: {it}")));
        AssetGraphValidationResult {
            valid: errors.is_empty(),
            root_ref: graph.root_ref.clone(),
            errors,
            warnings,
            graph: Some(graph),
        }
    }
}
