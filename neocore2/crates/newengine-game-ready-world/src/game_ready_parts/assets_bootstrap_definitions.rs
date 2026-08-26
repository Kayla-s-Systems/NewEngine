use super::*;

#[inline]
fn definition_requires_graph_resolution(mode: GameReadyDefinitionApplyMode) -> bool {
    matches!(mode, GameReadyDefinitionApplyMode::InstantiateMarker)
}

pub(super) fn instantiate_game_ready_definitions(
    world: &mut newengine_ecs::World,
    root: EntityId,
    definitions: &[GameReadyDefinitionInstanceSpec],
) {
    if definitions.is_empty() {
        return;
    }
    newengine_ulog_api::ulog::debug!(
        "definitions.runtime: game-ready definition batch count={} policy='.ymap placements declare apply_mode; .ytyp dependencies are graph inputs, not implicit render/spawn commands'",
        definitions.len()
    );
    for spec in definitions {
        if !definition_requires_graph_resolution(spec.apply_mode) {
            // MetadataOnly is already consumed by the owning domain during profile bootstrap.
            // Resolving the full dependency graph here is both redundant and potentially huge
            // (for example a player definition can expand into dozens of NEMAT/YTD nodes).
            // Generic instantiation must not turn descriptive metadata into launch-blocking I/O.
            newengine_ulog_api::ulog::debug!(
                "definitions.runtime: metadata-only definition_ref='{}' graph_resolution='skipped' apply_mode='{}' policy='domain metadata already consumed; no generic ECS/render marker and no recursive graph hydration'",
                spec.definition_ref,
                spec.apply_mode.as_str()
            );
            continue;
        }

        let graph = resolve_game_ready_asset_graph(&spec.definition_ref).unwrap_or_else(|| {
            newengine_model_domain_api::AssetGraphResolver::resolve_root_ref(&spec.definition_ref)
        });
        let transform = newengine_engine_runtime::world_authoring::DefinitionInstantiateTransform {
            translation: [spec.position.x, spec.position.y, spec.position.z],
            rotation_ypr: spec.rotation_ypr,
            scale: [spec.scale.x, spec.scale.y, spec.scale.z],
        };
        let (entity, trace) = newengine_engine_runtime::world_authoring::instantiate_definition(
            world,
            Some(root),
            spec.definition_ref.clone(),
            transform,
            graph,
        );
        newengine_ulog_api::ulog::debug!(
            "definitions.runtime: instantiated marker definition_ref='{}' entity={:?} nodes={} missing={} render_drawables={} materials={} textures={} physics_refs={} result='{}' apply_mode='{}'",
            trace.definition_ref,
            entity,
            trace.resolved_graph.nodes.len(),
            trace.resolved_graph.missing_refs.len(),
            trace.render_packet_request.drawable_refs.len(),
            trace.render_packet_request.material_refs.len(),
            trace.render_packet_request.texture_refs.len(),
            trace.physics_declaration.collision_refs.len() + trace.physics_declaration.physics_refs.len(),
            trace.apply_result,
            spec.apply_mode.as_str()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_only_definitions_skip_full_graph_resolution() {
        assert!(!definition_requires_graph_resolution(
            GameReadyDefinitionApplyMode::MetadataOnly
        ));
        assert!(definition_requires_graph_resolution(
            GameReadyDefinitionApplyMode::InstantiateMarker
        ));
    }
}
