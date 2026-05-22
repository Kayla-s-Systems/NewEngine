use super::*;

impl SceneBridge {
    #[inline]
    pub fn cmd_new_scene(&self) {
        self.queue.lock().cmds.push(SceneCommand::NewScene);
    }

    #[inline]
    pub fn cmd_load_scene_asset(&self, asset: SceneAsset) {
        self.queue.lock().cmds.push(SceneCommand::LoadSceneAsset { asset });
    }

    #[inline]
    pub fn cmd_spawn_primitive(&self, id: PrimitiveId, name: String, position: Vec3) {
        let scale = if id == builtins::ID_PLANE {
            [10.0, 1.0, 10.0]
        } else {
            [1.0, 1.0, 1.0]
        };
        let color = if id == builtins::ID_PLANE {
            [0.35, 0.35, 0.38, 1.0]
        } else {
            [0.85, 0.85, 0.9, 1.0]
        };
        self.queue.lock().cmds.push(SceneCommand::SpawnPrimitive {
            id,
            name,
            position: [position.x, position.y, position.z],
            scale,
            color,
        });
    }

    #[inline]
    pub fn cmd_spawn_directional_light(&self, name: String, position: Vec3, direction_ws: Vec3) {
        let d = direction_ws.normalize_or_zero();
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SpawnDirectionalLight {
                name,
                position: [position.x, position.y, position.z],
                direction_ws: [d.x, d.y, d.z],
            });
    }

    #[inline]
    pub fn cmd_spawn_point_light(&self, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnPointLight {
            name,
            position: [position.x, position.y, position.z],
        });
    }

    #[inline]
    pub fn cmd_spawn_player(&self, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnPlayer {
            name,
            position: [position.x, position.y, position.z],
        });
    }


    #[inline]
    pub fn cmd_spawn_imported_asset(&self, descriptor: SceneImportedAssetDescriptor, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnImportedAsset {
            descriptor,
            name,
            position: [position.x, position.y, position.z],
        });
    }

    #[inline]
    pub fn cmd_instantiate_definition(&self, definition_ref: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::InstantiateDefinition {
            definition_ref,
            position: [position.x, position.y, position.z],
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        });
    }

    #[inline]
    pub fn cmd_instantiate_definition_with_transform(
        &self,
        definition_ref: String,
        position: Vec3,
        rotation_ypr: (f32, f32, f32),
        scale: Vec3,
    ) {
        self.queue.lock().cmds.push(SceneCommand::InstantiateDefinition {
            definition_ref,
            position: [position.x, position.y, position.z],
            rotation_ypr: [rotation_ypr.0, rotation_ypr.1, rotation_ypr.2],
            scale: [scale.x, scale.y, scale.z],
        });
    }

    #[inline]
    pub fn cmd_set_transform(
        &self,
        entity: EntityId,
        position: Vec3,
        rotation_ypr: (f32, f32, f32),
        scale: Vec3,
    ) {
        self.queue.lock().cmds.push(SceneCommand::SetTransform {
            entity,
            position: [position.x, position.y, position.z],
            rotation_ypr: [rotation_ypr.0, rotation_ypr.1, rotation_ypr.2],
            scale: [scale.x, scale.y, scale.z],
        });
    }

    #[inline]
    pub fn cmd_set_primitive_color(&self, entity: EntityId, color: [f32; 4]) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetPrimitiveColor { entity, color });
    }

    #[inline]
    pub fn cmd_set_material(&self, entity: EntityId, material: MaterialId) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetMaterial { entity, material });
    }

    #[inline]
    pub fn cmd_update_material(&self, material: MaterialId, desc: MaterialDescriptor) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::UpdateMaterial { material, desc });
    }

    #[inline]
    pub fn cmd_set_ambient_light(&self, color: [f32; 3], intensity: f32) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetAmbientLight { color, intensity });
    }

    #[inline]
    pub fn cmd_set_directional_light(
        &self,
        entity: EntityId,
        direction_ws: Vec3,
        color: [f32; 3],
        intensity: f32,
    ) {
        let d = direction_ws.normalize_or_zero();
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetDirectionalLight {
                entity,
                direction_ws: [d.x, d.y, d.z],
                color,
                intensity,
            });
    }

    #[inline]
    pub fn cmd_set_point_light(
        &self,
        entity: EntityId,
        color: [f32; 3],
        intensity: f32,
        range: f32,
    ) {
        self.queue.lock().cmds.push(SceneCommand::SetPointLight {
            entity,
            color,
            intensity,
            range,
        });
    }

    #[inline]
    pub fn cmd_set_physics_body(&self, entity: EntityId, body: PhysicsBodyDesc) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetPhysicsBody { entity, body });
    }

    #[inline]
    pub fn cmd_clear_physics_body(&self, entity: EntityId) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::ClearPhysicsBody { entity });
    }


    #[inline]
    pub fn cmd_set_parent(&self, child: EntityId, parent: Option<EntityId>) {
        self.queue.lock().cmds.push(SceneCommand::SetParent { child, parent });
    }

    #[inline]
    pub fn cmd_set_display_visibility(&self, entity: EntityId, mode: DisplayMode) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetDisplayVisibility { entity, mode });
    }

    #[inline]
    pub fn cmd_set_play_mode(&self, mode: GameRunMode) {
        self.queue.lock().cmds.push(SceneCommand::SetPlayMode { mode });
    }



}
