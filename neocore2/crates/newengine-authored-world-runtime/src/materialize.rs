use newengine_assets_api::MapPlacementV1;
use newengine_definitions_runtime::DefinitionEntryV1;
use newengine_engine_runtime::gameplay::{DisplayMode, ModelRenderComponent};
use newengine_engine_runtime::scene_bridge::{
    SceneImportedAssetAssemblyDescriptor, SceneImportedAssetAssemblyKind,
    SceneImportedAssetDescriptor, SceneImportedAssetKind, SceneImportedAssetRepresentation,
};
use newengine_math::Vec3;
use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};
use newengine_scene::{spawn_named, DefinitionRef, Scene};
use newengine_transform::{set_parent, Transform};

use crate::AuthoredWorldBootstrapStats;

pub(crate) fn materialize_map(
    scene: &mut Scene,
    map: &super::loader::LoadedAuthoredMap,
) -> Result<(newengine_ecs::EntityId, AuthoredWorldBootstrapStats), String> {
    let root = newengine_engine_runtime::world_authoring::ensure_scene_root(scene);
    let mut stats = AuthoredWorldBootstrapStats {
        cells: map.cells.len(),
        ..Default::default()
    };
    let mut primary = None;

    for resolved in &map.cells {
        for placement in resolved
            .cell
            .placements
            .iter()
            .filter(|placement| placement.enabled)
        {
            let requires_definition = super::loader::placement_requires_definition(placement)?;
            let definition = if requires_definition {
                Some(
                    map.definitions
                        .get(&placement.definition_ref)
                        .ok_or_else(|| {
                            format!(
                                "authored-world definition cache miss ref='{}'",
                                placement.definition_ref
                            )
                        })?,
                )
            } else {
                None
            };
            let entity = materialize_placement(scene, root, placement, definition)?;
            primary.get_or_insert(entity);
            stats.placements += 1;
            if definition.is_some_and(|definition| !definition.refs.drawable_refs.is_empty()) {
                stats.model_actors += 1;
            } else {
                stats.definition_markers += 1;
            }
        }
    }

    let summary = newengine_engine_runtime::world_authoring::validate_scene_objects(
        scene.world_mut(),
        "authored-world.bootstrap",
    );
    newengine_ulog_api::ulog::info!(
        "authored world: materialized map='{}' map_id='{}' cells={} placements={} models={} markers={} scene_objects_checked={} repaired={}",
        map.map_ref, map.index.map_id, stats.cells, stats.placements, stats.model_actors,
        stats.definition_markers, summary.checked, summary.repaired,
    );
    Ok((primary.unwrap_or(root), stats))
}

fn materialize_placement(
    scene: &mut Scene,
    root: newengine_ecs::EntityId,
    placement: &MapPlacementV1,
    definition: Option<&DefinitionEntryV1>,
) -> Result<newengine_ecs::EntityId, String> {
    let position = Vec3::new(
        placement.transform.position[0],
        placement.transform.position[1],
        placement.transform.position[2],
    );
    let scale = Vec3::new(
        placement.transform.scale[0],
        placement.transform.scale[1],
        placement.transform.scale[2],
    );
    let rotation = placement.transform.rotation_ypr;
    let transform =
        Transform::from_yaw_pitch_roll(position, rotation[0], rotation[1], rotation[2], scale);
    let world = scene.world_mut();
    let entity = spawn_named(world, format!("Map/{}", placement.id));
    if !set_parent(world, entity, Some(root)) {
        return Err(format!(
            "authored-world failed to parent placement id='{}'",
            placement.id
        ));
    }
    let _ = world.insert(entity, transform);
    let _ = world.insert(entity, DefinitionRef(placement.definition_ref.clone()));

    let half_extents = Vec3::new(
        scale.x.abs().max(0.5),
        scale.y.abs().max(0.5),
        scale.z.abs().max(0.5),
    );
    newengine_engine_runtime::world_authoring::attach_scene_object(
        world,
        entity,
        position,
        half_extents,
    );

    if let Some((definition, drawable_ref)) = definition.and_then(|definition| {
        definition
            .refs
            .drawable_refs
            .first()
            .map(|drawable| (definition, drawable))
    }) {
        let _ = world.insert(entity, ModelRenderComponent::new(drawable_ref.clone()));
        let _ = world.insert(entity, definition.model_explanation.render_options.clone());
        let with_collision = !definition.refs.collision_refs.is_empty()
            || !definition.refs.physics_refs.is_empty()
            || matches!(
                definition
                    .model_explanation
                    .collision_policy
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "static_mesh" | "triangle_mesh" | "mesh" | "box"
            );
        let dynamic = placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("dynamic_physics");
        let descriptor = SceneImportedAssetDescriptor {
            logical_path: drawable_ref.clone(),
            import_kind: SceneImportedAssetKind::StaticMesh,
            representation: SceneImportedAssetRepresentation::PrimitiveCube,
            assembler_key: "builtin.static_mesh_actor".to_owned(),
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::StaticMeshActor,
                primitive_id: newengine_primitives::builtins::ID_CUBE,
                display_mode: DisplayMode::Both,
                with_collision,
                dynamic_collision: dynamic,
            },
            default_scale: [scale.x, scale.y, scale.z],
            tint: [1.0, 1.0, 1.0, 1.0],
        };
        let _ = world.insert(entity, descriptor);
        if with_collision {
            let shape = CollisionShapeDesc::Box {
                half_extents: [half_extents.x, half_extents.y, half_extents.z],
            };
            let body = if dynamic {
                PhysicsBodyDesc::dynamic_solid(shape)
            } else {
                PhysicsBodyDesc::static_solid(shape)
            };
            let _ = world.insert(entity, body);
        }
    }

    Ok(entity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_only_placement_never_synthesizes_render_actor() {
        let mut scene = Scene::new();
        let root = newengine_engine_runtime::world_authoring::ensure_scene_root(&mut scene);
        let placement = MapPlacementV1 {
            id: "metadata".to_owned(),
            definition_ref: "definitions/world.ytyp@metadata".to_owned(),
            apply_mode: "metadata_only".to_owned(),
            ..Default::default()
        };
        let entity = materialize_placement(&mut scene, root, &placement, None).expect("marker");
        let world = scene.world();
        assert!(world.get::<ModelRenderComponent>(entity).is_none());
        assert_eq!(
            world.get::<DefinitionRef>(entity).unwrap().0,
            placement.definition_ref
        );
    }

    #[test]
    fn placement_transform_is_preserved_for_generic_model_actor() {
        let mut scene = Scene::new();
        let root = newengine_engine_runtime::world_authoring::ensure_scene_root(&mut scene);
        let placement = MapPlacementV1 {
            id: "tree".to_owned(),
            definition_ref: "definitions/world.ytyp@tree".to_owned(),
            transform: newengine_assets_api::MapTransformV1 {
                position: [4.0, 2.0, -3.0],
                rotation_ypr: [0.5, 0.1, 0.0],
                scale: [2.0, 3.0, 4.0],
            },
            ..Default::default()
        };
        let mut definition = DefinitionEntryV1::default();
        definition
            .refs
            .drawable_refs
            .push("models/tree.ydd@tree".to_owned());
        let entity = materialize_placement(&mut scene, root, &placement, Some(&definition))
            .expect("placement");
        let world = scene.world();
        let transform = world.get::<Transform>(entity).expect("transform");
        assert_eq!(transform.position, Vec3::new(4.0, 2.0, -3.0));
        assert_eq!(transform.scale, Vec3::new(2.0, 3.0, 4.0));
        assert!(world.get::<ModelRenderComponent>(entity).is_some());
        assert_eq!(
            world.get::<DefinitionRef>(entity).unwrap().0,
            placement.definition_ref
        );
    }
}
