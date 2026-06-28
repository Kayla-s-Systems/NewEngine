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
    pub fn selection_authority_handle(&self) -> Option<newengine_entity_api::EntityHandle> {
        *self.selection_authority.lock()
    }

    #[inline]
    pub fn set_selection(&self, id: Option<EntityId>) {
        *self.selection.lock() = id;
        let authority = id.and_then(|entity| {
            let scene = self.scene.read();
            crate::authority::current_entity_authority_map(scene.world())
                .and_then(|map| map.provider_for_native(entity))
        });
        *self.selection_authority.lock() = authority;
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
