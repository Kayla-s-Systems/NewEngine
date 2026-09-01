use std::collections::BTreeSet;

use newengine_ecs::EntityId;
use newengine_materials::MaterialDescriptor;
use newengine_primitives::Primitive;
use newengine_scene::{spawn_named, Scene};

use crate::scene_bridge::{
    apply_primitive_instance, ensure_primitive_base, primitive_bounds, SceneBridge,
};
use newengine_gameplay_world_runtime::gameplay::{DisplayMode, DisplayVisibility};

#[derive(Default)]
pub struct EditorViewportSceneAdapter {
    gizmo_entities: [Option<EntityId>; newengine_editor_viewport_runtime::GIZMO_HANDLE_COUNT],
}

impl EditorViewportSceneAdapter {
    pub fn sync_gizmo_geometry(
        &mut self,
        controller: &newengine_editor_viewport_runtime::EditorViewportController,
        scene_bridge: &SceneBridge,
        scene: &mut Scene,
        selected: Option<EntityId>,
        selection_radius: f32,
    ) {
        let specs = controller.gizmo_specs(scene.world(), selected, selection_radius);
        if specs.is_empty() {
            self.remove_gizmos(scene.world_mut());
            return;
        }
        let desired = specs
            .iter()
            .map(|spec| spec.handle.index())
            .collect::<BTreeSet<_>>();
        let materials = scene_bridge.materials();
        let mats = materials.read();
        let default_mat = mats.register_named("EditorGizmo", MaterialDescriptor::default());
        let primitives = scene_bridge.primitives();
        let prims = primitives.read();
        let world = scene.world_mut();

        for (index, slot) in self.gizmo_entities.iter_mut().enumerate() {
            if desired.contains(&index) {
                continue;
            }
            if let Some(entity) = slot.take() {
                if world.exists(entity) {
                    let _ = world.despawn(entity);
                }
            }
        }

        for spec in specs {
            let index = spec.handle.index();
            let entity = match self.gizmo_entities[index].filter(|entity| world.exists(*entity)) {
                Some(entity) => entity,
                None => {
                    let entity = spawn_named(world, format!("__EditorGizmo{}", spec.handle.name()));
                    self.gizmo_entities[index] = Some(entity);
                    entity
                }
            };
            let _ = world.insert(
                entity,
                Primitive {
                    id: spec.primitive,
                    color: spec.color,
                },
            );
            let _ = world.insert(
                entity,
                newengine_editor_viewport_runtime::EditorGizmoAxisComponent {
                    handle: spec.handle,
                },
            );
            let _ = world.insert(
                entity,
                DisplayVisibility {
                    mode: DisplayMode::RuntimeHidden,
                },
            );
            let _ = world.insert(entity, spec.render_options);
            if let Some(bounds) = primitive_bounds(&prims, spec.primitive) {
                let _ = world.insert(entity, bounds);
            }
            ensure_primitive_base(world, entity, default_mat);
            apply_primitive_instance(world, &mats, entity, default_mat, spec.color);
            let _ = world.insert(entity, spec.transform);
        }
    }

    pub fn clear_runtime_geometry(&mut self, world: &mut newengine_ecs::World) {
        self.remove_gizmos(world);
    }

    fn remove_gizmos(&mut self, world: &mut newengine_ecs::World) {
        for entity in &mut self.gizmo_entities {
            if let Some(id) = entity.take() {
                if world.exists(id) {
                    let _ = world.despawn(id);
                }
            }
        }
    }
}
