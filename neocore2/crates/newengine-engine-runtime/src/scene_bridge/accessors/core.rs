use super::*;

impl SceneBridge {
    #[inline]
    pub fn register_imported_asset_assembler(&self, assembler: SceneImportedAssetAssembler) {
        let mut registry = self.asset_assemblers.write();
        if let Some(existing) = registry.iter_mut().find(|it| it.key == assembler.key) {
            *existing = assembler;
            return;
        }
        registry.push(assembler);
    }

    #[inline]
    pub fn imported_asset_assemblers_snapshot(&self) -> Vec<SceneImportedAssetAssembler> {
        self.asset_assemblers.read().clone()
    }

    #[inline]
    pub fn authority_bridge(
        &self,
    ) -> std::sync::Arc<crate::authority::RuntimeWorldAuthorityBridge> {
        std::sync::Arc::clone(&self.authority)
    }

    #[inline]
    pub fn authority_snapshot(
        &self,
    ) -> newengine_runtime_host::world_authority::WorldAuthoritySnapshot {
        self.authority.detect()
    }

    #[inline]
    pub fn selection(&self) -> Option<EntityId> {
        *self.selection.lock()
    }

    #[inline]
    pub fn selections(&self) -> Vec<EntityId> {
        self.selection_set.lock().clone()
    }

    #[inline]
    pub fn selection_authority_handle(&self) -> Option<newengine_entity_api::EntityHandle> {
        *self.selection_authority.lock()
    }

    pub fn set_selection(&self, id: Option<EntityId>) {
        let changed = {
            let mut selection = self.selection.lock();
            let mut set = self.selection_set.lock();
            let next_set: Vec<EntityId> = id.into_iter().collect();
            let changed = *selection != id || *set != next_set;
            *selection = id;
            *set = next_set;
            changed
        };
        if changed {
            self.refresh_selection_authority_and_inspector(id);
        }
    }

    pub fn toggle_selection(&self, id: EntityId) {
        let primary = {
            let mut set = self.selection_set.lock();
            if let Some(index) = set.iter().position(|candidate| *candidate == id) {
                set.remove(index);
            } else {
                set.push(id);
            }
            let primary = set.last().copied();
            *self.selection.lock() = primary;
            primary
        };
        self.refresh_selection_authority_and_inspector(primary);
    }

    pub fn replace_selections(&self, ids: impl IntoIterator<Item = EntityId>) {
        let mut unique = Vec::new();
        for id in ids {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        let primary = unique.last().copied();
        *self.selection_set.lock() = unique;
        *self.selection.lock() = primary;
        self.refresh_selection_authority_and_inspector(primary);
    }

    fn refresh_selection_authority_and_inspector(&self, id: Option<EntityId>) {
        let authority = id.and_then(|entity| {
            let scene = self.scene.read();
            crate::authority::current_entity_authority_map(scene.world())
                .and_then(|map| map.provider_for_native(entity))
        });
        *self.selection_authority.lock() = authority;
        self.publish_inspector_state(id);
    }

    #[inline]
    pub fn in_game_editor_enabled(&self) -> bool {
        *self.in_game_editor_enabled.lock()
    }

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
        if !changed {
            return enabled;
        }
        if !enabled {
            *self.selection.lock() = None;
            self.selection_set.lock().clear();
            *self.selection_authority.lock() = None;
            let snapshot = self.inspector_snapshot_json(None);
            publish_inspector_snapshot_to_surface(&snapshot, GAME_HUD_SURFACE_ID);
        } else {
            crate::ui_gateway::set_surface_visible(EDITOR_INSPECTOR_SURFACE_ID, false);
        }
        self.publish_in_game_editor_state(enabled);
        if enabled {
            self.publish_inspector_state(self.selection());
        }
        newengine_ulog_api::ulog::info!(
            "in-game editor: mode={} source='engine.scene' center_pick={} gameplay_input_gated={}",
            if enabled { "edit" } else { "play" },
            enabled,
            enabled,
        );
        enabled
    }

    #[inline]
    pub fn toggle_in_game_editor(&self) -> bool {
        self.set_in_game_editor_enabled(!self.in_game_editor_enabled())
    }

    #[inline]
    pub fn play_mode(&self) -> GameRunMode {
        *self.play_mode.lock()
    }

    #[inline]
    pub fn materials_snapshot(&self) -> Vec<(String, MaterialId)> {
        let reg = self.materials.read();
        let mut out: Vec<(String, MaterialId)> = reg
            .snapshot()
            .into_iter()
            .map(|it| (it.name, it.id))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[inline]
    pub fn primitives_snapshot(&self) -> Vec<(String, PrimitiveId)> {
        let reg = self.primitives.read();
        let mut out: Vec<(String, PrimitiveId)> = reg
            .ids()
            .filter_map(|id| reg.name(id).map(|n| (n.to_string(), id)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}
